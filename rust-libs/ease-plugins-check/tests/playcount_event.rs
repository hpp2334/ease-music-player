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

    /// The harness booted: an initialized database (all sqlx work rides
    /// the SHARED ease runtime — the very runtime the `ease.db` rows
    /// block_on from the worker lanes; a pool created under a foreign
    /// runtime times out on acquire) plus a live app thread.
    fn boot() -> (
        Arc<RpcBus>,
        mpsc::Sender<Cmd>,
        Arc<ease_client_backend::repositories::core::DatabaseServer>,
        std::path::PathBuf,
    ) {
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
        ease_client_tokio::tokio_runtime()
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
        (bus, cmd_tx, db, db_dir)
    }

    /// One `music:play` dispatch, exactly as `RpcClient::emit_event` does
    /// it: stash `{"type","payload"}` then fire the subscribed export.
    /// Asserts the ok envelope.
    fn fire_play(bus: &RpcBus, cmd_tx: &mpsc::Sender<Cmd>, payload: &str) {
        let (token_tx, token_rx) = mpsc::sync_channel(1);
        cmd_tx.send(Cmd::Token(token_tx)).ok();
        let token = token_rx.recv().expect("token");
        assert_ne!(token, 0, "the backend must boot (start -> opaque)");

        bus.stash_request(format!("{{\"type\":\"music:play\",\"payload\":{payload}}}"));
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        cmd_tx
            .send(Cmd::CallStr("op_on_music_play".into(), token, reply_tx))
            .expect("driver alive");
        let reply = reply_rx.recv().expect("reply");
        assert!(
            reply.as_deref().is_ok_and(|r| r.contains("\"ok\":true")),
            "the event handler must accept the host payload — got: {reply:?}"
        );
    }

    /// Every row in the plugin's `plays:` keys, as `(key, row)`.
    fn all_play_rows(
        db: &Arc<ease_client_backend::repositories::core::DatabaseServer>,
    ) -> Vec<(String, String)> {
        let ease_rt = ease_client_tokio::tokio_runtime();
        let keys = ease_rt
            .block_on(db.plugin_kv_list_keys("com.ease.playcount", "plays:"))
            .expect("list plays keys");
        let mut out = Vec::new();
        for info in &keys {
            let values = ease_rt
                .block_on(db.plugin_kv_multi_get_all("com.ease.playcount", &info.key))
                .expect("read plays rows");
            for v in values {
                out.push((info.key.clone(), v));
            }
        }
        out
    }

    /// `plays:YYYY-MM-DD` for a days-since-epoch count (Howard Hinnant's
    /// civil_from_days) — mirrors the plugin's `day_key_from_ms`.
    fn utc_day_key(days: i64) -> String {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("plays:{y:04}-{m:02}-{d:02}")
    }

    /// The payload EXACTLY as the host emits it: `musicId` is a NUMBER.
    const MUSIC_PLAY_PAYLOAD: &str =
        r#"{"musicId": 4242, "title": "Some Song", "ts": 1767000000000}"#;

    #[test]
    fn playcount_records_the_real_host_event_payload() {
        let (bus, cmd_tx, db, db_dir) = boot();

        fire_play(&bus, &cmd_tx, MUSIC_PLAY_PAYLOAD);

        // The play row must land in the plugin-scoped KV.
        let rows = all_play_rows(&db);
        assert!(!rows.is_empty(), "no `plays:` day key was written");
        let hit = rows.iter().find(|(_, row)| row.contains("4242"));
        let (_, row) = hit.expect(
            "the play row must land in the plugin KV — a miss means the \
             handler dropped the host payload (number-vs-string decode?)",
        );
        assert!(row.contains("Some Song"), "row must carry the title: {row}");
        assert!(row.contains("musicId"), "row shape unexpected: {row}");

        let _ = std::fs::remove_dir_all(&db_dir);
    }

    /// The TS keyed day buckets by device-LOCAL midnight; this port keys
    /// by UTC (no TZ row in rut). The first-event repair must MOVE legacy
    /// local-day rows to the UTC day their own `ts` names — otherwise an
    /// evening play (UTC+8: 16:00-24:00 local) is keyed one day ahead and
    /// surfaces under "Today" a day late, forever.
    #[test]
    fn legacy_local_day_rows_are_rekeyed_to_utc() {
        let (bus, cmd_tx, db, db_dir) = boot();
        let ease_rt = ease_client_tokio::tokio_runtime();
        let pid = "com.ease.playcount";

        // Day boundaries relative to now, so the 400-day repair window
        // always covers the fixtures.
        let today = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
            / 86_400;

        // A UTC+8 evening play: happened 20:00 UTC today (UTC day = today)
        // but a local-midnight writer keyed it under TOMORROW's day.
        let ts_ms = today * 86_400_000 + 20 * 3_600_000;
        let legacy_row = format!("{{\"musicId\":77,\"title\":\"Old Song\",\"ts\":{ts_ms}}}");
        let legacy_key = utc_day_key(today + 1);
        ease_rt
            .block_on(db.plugin_kv_multi_append(pid, &legacy_key, &legacy_row))
            .expect("seed legacy row");

        // A row already under its correct UTC day must stay untouched.
        let stable_ts = (today - 5) * 86_400_000 + 12 * 3_600_000;
        let stable_row = format!("{{\"musicId\":88,\"title\":\"Stable\",\"ts\":{stable_ts}}}");
        let stable_key = utc_day_key(today - 5);
        ease_rt
            .block_on(db.plugin_kv_multi_append(pid, &stable_key, &stable_row))
            .expect("seed stable row");

        // Any play event triggers the once-per-install repair pass.
        fire_play(
            &bus,
            &cmd_tx,
            &format!(
                "{{\"musicId\":42,\"title\":\"Now\",\"ts\":{}}}",
                today * 86_400_000 + 21 * 3_600_000
            ),
        );

        let rows = all_play_rows(&db);

        // The legacy row MOVED to its UTC day (today).
        assert!(
            rows
                .iter()
                .any(|(k, r)| k == &utc_day_key(today) && r.contains("Old Song")),
            "the legacy local-day row must be re-keyed to its UTC day; rows: {rows:?}"
        );

        // Its old (tomorrow) key no longer answers.
        assert!(
            !rows.iter().any(|(k, _)| k == &legacy_key),
            "the legacy day key must be emptied; rows: {rows:?}"
        );

        // The already-correct row survives exactly once (the rewrite must
        // not duplicate untouched keys).
        let stable_count = rows
            .iter()
            .filter(|(k, r)| k == &stable_key && r.contains("Stable"))
            .count();
        assert_eq!(
            stable_count, 1,
            "the untouched row must survive exactly once; rows: {rows:?}"
        );

        let _ = std::fs::remove_dir_all(&db_dir);
    }
}
