//! Regression gate for the host→plugin event payload contract: dispatch a
//! `music:play` event shaped EXACTLY as `PluginEvent.MusicPlay.toJsonElement()`
//! produces (note `musicId` rides as a JSON **number** — `MusicId.value:
//! Long`) into the real playcount backend, over an initialized database,
//! and require the play row to land in the plugin KV.
//!
//! This is the hole the first migration fell through: the handler decoded
//! `musicId` with a string reader, which fails sticky on a number token —
//! every play was silently dropped (fire-and-forget dispatch) while the
//! whole suite stayed green.
//!
//! Harness: the harness app lives on its own thread (it is `!Send`) and is
//! stamped with the `PluginId` instance-data slot — `ease.db` rows are
//! plugin-scoped, and without the stamp every scoped row traps. SQLx calls
//! (db init + asserts) run inside the tokio test.

#[cfg(test)]
mod playcount_event {
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId};
    use ease_client_backend::ctx::BackendContext;
    use ease_client_backend::repositories::secret::SecretStore;
    use ease_tur_rpc::{RpcBus, TurRpcBusPlugin};
    use std::sync::mpsc;
    use std::sync::Arc;
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::TurRuntime;
    use tur_engine::TurStdPlugin;
    use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
    use tur_native::NativeFontLoader;

    enum Cmd {
        /// Stash is done test-side (`RpcBus::stash_request`); this drives
        /// the named export with the context token and answers the reply.
        CallStr(String, u64, mpsc::SyncSender<Result<String, String>>),
        Token(mpsc::SyncSender<u64>),
    }

    /// The app side: build the runtime + a headless, PluginId-stamped
    /// instance, load the backend, then serve entry-call commands.
    fn spawn_app(
        cx: BackendContext,
        bus: std::sync::Arc<RpcBus>,
        source: String,
        ready_tx: mpsc::Sender<()>,
        rx: mpsc::Receiver<Cmd>,
    ) {
        std::thread::spawn(move || {
            let driver = TestSchedulerDriver::new();
            let pool = WorkerPoolHandle::new("test", 2);
            let runtime = TurRuntime::builder()
                .worker_spawner(driver.worker_spawner())
                .host_loop(driver.host_loop())
                .font_loader(Arc::new(NativeFontLoader::new()))
                .clock(Arc::new(MutexFixedClock::new(0)))
                .worker_pool(pool.clone())
                .plugin(TurStdPlugin)
                .plugin(TurRpcBusPlugin::new(Some(bus)))
                .plugin(EaseMusicPlugin::new(cx))
                .build()
                .expect("runtime build");
            let (app, _looper) = runtime
                .app_builder()
                .worker_pool(pool)
                .instance_data(|icx| {
                    icx.define::<PluginId>(PluginId::new("com.ease.playcount"));
                })
                .build_headless((0.0, 0.0))
                .expect("headless app build");
            futures::executor::block_on(app.load_rut_module(source.as_str()))
                .expect("playcount backend loads");
            ready_tx.send(()).expect("test still waiting");
            for cmd in rx {
                match cmd {
                    Cmd::CallStr(name, cx, rtx) => {
                        let _ = rtx.send(
                            futures::executor::block_on(
                                app.call_rut_entry_cx_str(&name, cx),
                            )
                            .map_err(|e| e.to_string()),
                        );
                    }
                    Cmd::Token(rtx) => {
                        let _ = rtx.send(futures::executor::block_on(app.rut_start_answer()));
                    }
                }
            }
        });
    }

    /// The payload EXACTLY as the host emits it: `musicId` is a NUMBER.
    const MUSIC_PLAY_PAYLOAD: &str =
        r#"{"musicId": 4242, "title": "Some Song", "ts": 1767000000000}"#;

    #[test]
    fn playcount_records_the_real_host_event_payload() {
        let db_dir = std::env::temp_dir().join(format!(
            "ease-playcount-check-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&db_dir).expect("temp db dir");

        let cx = BackendContext::new();
        let db = cx.database_server().clone();
        // All sqlx work rides the SHARED ease runtime — the very runtime
        // the `ease.db` rows block_on from the worker lanes. A pool created
        // under a foreign runtime (e.g. #[tokio::test]) times out on
        // acquire from here.
        let ease_rt = ease_client_tokio::tokio_runtime();
        ease_rt
            .block_on(db.init(db_dir.to_string_lossy().to_string()))
            .expect("db init");

        let source = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../plugins/com.ease.playcount/backend.rut"
        ))
        .expect("playcount backend.rut readable");

        let bus = RpcBus::new();
        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<()>();
        spawn_app(cx, bus.clone(), source, ready_tx, cmd_rx);
        ready_rx.recv().expect("driver thread booted");

        // The event dispatch, exactly as RpcClient::emit_event does it:
        // stash `{"type","payload"}` then fire the subscribed export.
        let (token_tx, token_rx) = mpsc::sync_channel(1);
        cmd_tx.send(Cmd::Token(token_tx)).ok();
        let token = token_rx.recv().expect("token");
        assert_ne!(token, 0, "the backend must boot (start -> opaque)");

        bus.stash_request(format!(
            "{{\"type\":\"music:play\",\"payload\":{MUSIC_PLAY_PAYLOAD}}}"
        ));
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        cmd_tx
            .send(Cmd::CallStr("op_on_music_play".into(), token, reply_tx))
            .expect("driver alive");
        let reply = reply_rx.recv().expect("reply");
        assert!(
            reply.as_deref().is_ok_and(|r| r.contains("\"ok\":true")),
            "the event handler must accept the real host payload — got: {reply:?}"
        );

        // The play row must land in the plugin-scoped KV.
        let keys = ease_rt
            .block_on(db.plugin_kv_list_keys("com.ease.playcount", "plays:"))
            .expect("list plays keys");
        assert!(!keys.is_empty(), "no `plays:` day key was written");
        let mut rows = Vec::new();
        for info in &keys {
            let values = ease_rt
                .block_on(db.plugin_kv_multi_get_all("com.ease.playcount", &info.key))
                .expect("read plays rows");
            rows.extend(values);
        }
        let hit = rows.iter().find(|row| row.contains("4242"));
        let row = hit.expect(
            "the play row must land in the plugin KV — a miss means the \
             handler dropped the host payload (number-vs-string decode?)",
        );
        assert!(row.contains("Some Song"), "row must carry the title: {row}");
        assert!(row.contains("musicId"), "row shape unexpected: {row}");

        let _ = std::fs::remove_dir_all(&db_dir);
    }
}
