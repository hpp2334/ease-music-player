//! Regression gate for the host→plugin event payload contract: dispatch a
//! `music:play` event shaped EXACTLY as `PluginEvent.MusicPlay.toJsonElement()`
//! produces (note `musicId` rides as a JSON **number** — `MusicId.value:
//! Long`) into the real playcount backend, through the host's own lane
//! (`RpcClient.emit_event` → the bind-table's `event:music:play` key), over
//! an initialized database, and require the play row to land in the plugin KV.
//!
//! This is the hole the first migration fell through: the handler decoded
//! `musicId` with a string reader, which fails sticky on a number token —
//! every play was silently dropped (fire-and-forget dispatch) while the
//! whole suite stayed green.
//!
//! Harness: the harness app lives on its own thread (it is `!Send`) and is
//! stamped with the minted `PluginHandle` (+ its `RpcIdentity` view) — the
//! emp services' identity — and loaded through the cx lane with that same
//! id. `ease.db` rows are plugin-scoped, and without the stamp every scoped
//! row traps. SQLx calls (db init + asserts) run inside the tokio test.

#[cfg(test)]
mod playcount_event {
    use ease_client_android::plugin_runtime::emp::PluginHandle;
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId, PluginInstance};
    use ease_client_backend::ctx::BackendContext;
    use ease_client_backend::services::plugin_pkg::{read_entry_source, EntrySource};
    use ease_tur_rpc::{DispatchAnswer, RpcClient, RutDriver, TurRpcBindPlugin};
    use std::path::Path;
    use std::sync::mpsc;
    use std::sync::Arc;
    use tur_engine::core::rut_runtime::RutModuleFiles;
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::TurRuntime;
    use tur_engine::TurStdPlugin;
    use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
    use tur_native::NativeFontLoader;

    enum Cmd {
        DispatchStr(
            String,
            u64,
            String,
            String,
            mpsc::SyncSender<Result<DispatchAnswer, String>>,
        ),
        DispatchBytes(
            String,
            u64,
            String,
            Vec<u8>,
            mpsc::SyncSender<Result<DispatchAnswer, String>>,
        ),
    }

    struct TestDriver {
        tx: mpsc::Sender<Cmd>,
    }

    impl RutDriver for TestDriver {
        fn call_dispatch_str(
            &self,
            name: &str,
            cx: u64,
            op: &str,
            args: &str,
        ) -> Result<DispatchAnswer, String> {
            let (rtx, rrx) = mpsc::sync_channel(1);
            self.tx
                .send(Cmd::DispatchStr(
                    name.to_string(),
                    cx,
                    op.to_string(),
                    args.to_string(),
                    rtx,
                ))
                .map_err(|_| "driver thread gone".to_string())?;
            rrx.recv().map_err(|_| "driver dropped".to_string())?
        }

        fn call_dispatch_bytes(
            &self,
            name: &str,
            cx: u64,
            op: &str,
            args: &[u8],
        ) -> Result<DispatchAnswer, String> {
            let (rtx, rrx) = mpsc::sync_channel(1);
            self.tx
                .send(Cmd::DispatchBytes(
                    name.to_string(),
                    cx,
                    op.to_string(),
                    args.to_vec(),
                    rtx,
                ))
                .map_err(|_| "driver thread gone".to_string())?;
            rrx.recv().map_err(|_| "driver dropped".to_string())?
        }
    }

    fn dispatch_answer(a: tur_engine::RutEntryAnswer) -> DispatchAnswer {
        match a {
            tur_engine::RutEntryAnswer::Nil => DispatchAnswer::Nil,
            tur_engine::RutEntryAnswer::Str(s) => DispatchAnswer::Str(s),
            tur_engine::RutEntryAnswer::Bytes(b) => DispatchAnswer::Bytes(b),
            other => panic!("unexpected dispatch answer: {other:?}"),
        }
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
    }

    /// The app side: build the runtime + a headless, PluginHandle-stamped
    /// instance, load the backend through the cx lane, then serve
    /// dispatch-entry commands.
    fn spawn_app(
        cx: BackendContext,
        source: EntrySource,
        ready_tx: mpsc::Sender<(u64, ())>,
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
                .plugin(TurRpcBindPlugin::new())
                .plugin(EaseMusicPlugin::new(cx))
                .build()
                .expect("runtime build");
            let handle = PluginHandle::mint(
                PluginId::new("com.ease.playcount"),
                PluginInstance(None),
            );
            let boot_cx = handle.id();
            let (app, _looper) = runtime
                .app_builder()
                .worker_pool(pool)
                .instance_data(move |icx| {
                    icx.define::<PluginHandle>(handle);
                    icx.define::<ease_tur_rpc::RpcIdentity>(ease_tur_rpc::RpcIdentity(boot_cx));
                })
                .build_headless((0.0, 0.0))
                .expect("headless app build");
            // A single file rides the single rail, a pkg tree the
            // engine's multi-file rail — the two shapes the scan can
            // register (parity with the lib harness's
            // `assert_loads_source`), both through the cx lane (the
            // bind-table contract's `start(cx)` identity).
            let loaded = match source {
                EntrySource::Single(src) => {
                    futures::executor::block_on(app.load_rut_module_cx(src.as_str(), boot_cx))
                }
                EntrySource::FileSet { root, mods } => futures::executor::block_on(
                    app.load_rut_module_files_cx(
                        RutModuleFiles {
                            root: Arc::from(root.as_str()),
                            mods: mods
                                .into_iter()
                                .map(|(path, text)| {
                                    (
                                        std::sync::Arc::from(path.as_str()),
                                        std::sync::Arc::from(text.as_str()),
                                    )
                                })
                                .collect(),
                        },
                        boot_cx,
                    ),
                ),
            };
            loaded.expect("playcount backend loads");
            ready_tx.send((boot_cx, ())).expect("test still waiting");
            for cmd in rx {
                match cmd {
                    Cmd::DispatchStr(name, cx, op, args, rtx) => {
                        let _ = rtx.send(
                            futures::executor::block_on(
                                app.call_rut_entry_cx_str_str(&name, cx, &op, &args),
                            )
                            .map(dispatch_answer)
                            .map_err(|e| e.to_string()),
                        );
                    }
                    Cmd::DispatchBytes(name, cx, op, args, rtx) => {
                        let _ = rtx.send(
                            futures::executor::block_on(
                                app.call_rut_entry_cx_str_bytes(&name, cx, &op, &args),
                            )
                            .map(dispatch_answer)
                            .map_err(|e| e.to_string()),
                        );
                    }
                }
            }
        });
    }

    /// The harness booted: an initialized database (all sqlx work rides
    /// the SHARED ease runtime — the very runtime the `ease.db` rows
    /// block_on from the worker lanes; a pool created under a foreign
    /// runtime times out on acquire) plus a live app thread, with the
    /// host client over the bind-table dispatch entry.
    fn boot() -> (
        RpcClient,
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

        // The entrypoint resolves through the manifest's `backend`
        // spelling via the scan's own lane (`read_entry_source`) — the
        // migrated plugin is a pkg tree; no flat `backend.rut` exists.
        let dir = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../plugins/com.ease.playcount"
        ));
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("manifest.json")).expect("manifest.json readable"),
        )
        .expect("manifest.json parses");
        let backend = manifest["backend"].as_str().expect("manifest backend field");
        let source = read_entry_source(dir, backend).expect("playcount backend entrypoint readable");

        let (cmd_tx, cmd_rx) = mpsc::channel::<Cmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<(u64, ())>();
        spawn_app(cx, source, ready_tx, cmd_rx);
        let (boot_cx, ()) = ready_rx.recv().expect("driver thread booted");
        (
            RpcClient::new(Arc::new(TestDriver { tx: cmd_tx }), boot_cx),
            db,
            db_dir,
        )
    }
    /// One `music:play` dispatch, exactly as the host emits it:
    /// `RpcClient::emit_event` → the bind-table's `event:music:play` key.
    fn fire_play(client: &RpcClient, payload: &str) {
        let payload: serde_json::Value =
            serde_json::from_str(payload).expect("payload json parses");
        runtime().block_on(client.emit_event("music:play", payload));
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
        let (client, db, db_dir) = boot();

        fire_play(&client, MUSIC_PLAY_PAYLOAD);

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
        let (client, db, db_dir) = boot();
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
            &client,
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
