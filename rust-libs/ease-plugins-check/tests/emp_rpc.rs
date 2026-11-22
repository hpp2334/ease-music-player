//! The phase-3 bind-table contract, end-to-end through `emp`'s `Rpc`
//! class: the fixture (the plan's exact shape — `start(cx)` + `rpc.bind`
//! + plain fns + the two dispatch entries) compiles and boots against the
//! full app plugin set, and `RpcClient`'s new dispatch lane serves it —
//! str ops, raw-bytes ops, the nil "not mine" arm, the trap prefix law,
//! the event fan-out — with the OLD stash contract's fixtures
//! (`storage_dispatch.rs` / `playcount_event.rs`) still green beside it.
//!
//! The storage ops additionally ride [`JsStorageBackend`] — the full
//! chunked-pull chain over the bind-table lane. The identity rides the
//! host-minted `PluginHandle` (stamped beside its `RpcIdentity` view);
//! `extract_rpc`'s boot gate accepts the entries as a new-style backend's
//! boot proof (no start answer exists on this contract).

#[cfg(test)]
mod emp_rpc {
    use ease_client_android::plugin_runtime::emp::PluginHandle;
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId, PluginInstance};
    use ease_client_backend::ctx::BackendContext;
    use ease_js_storage::JsStorageBackend;
    use ease_remote_storage::StorageBackend;
    use ease_tur_rpc::{DispatchAnswer, RpcBus, RpcClient, RpcError, RutDriver, TurRpcBusPlugin};
    use std::sync::mpsc;
    use std::sync::Arc;
    use tur_engine::core::plugin::Plugin;
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::{TurStdPlugin, TurRuntime};
    use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
    use tur_native::NativeFontLoader;

    const FIXTURE: &str = include_str!("emp_rpc_fixture.rut");

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
        Token(mpsc::SyncSender<u64>),
    }

    impl RutDriver for TestDriver {
        fn call_entry_str(&self, _name: &str, _cx: u64) -> Result<String, String> {
            Err("the bind-table fixture serves no stash-bus entries".to_string())
        }

        fn call_entry(&self, _name: &str, _cx: u64) -> Result<(), String> {
            Err("the bind-table fixture serves no stash-bus entries".to_string())
        }

        fn start_token(&self) -> u64 {
            let (rtx, rrx) = mpsc::sync_channel(1);
            self.tx.send(Cmd::Token(rtx)).ok();
            rrx.recv().unwrap_or(0)
        }

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

    struct TestDriver {
        tx: mpsc::Sender<Cmd>,
    }

    /// Boot the fixture the production headless shape: stamped with a
    /// minted [`PluginHandle`] (+ its [`ease_tur_rpc::RpcIdentity`] view),
    /// loaded through the phase-1 cx lane with that same id.
    fn boot() -> RpcClient {
        let cx = BackendContext::new();
        // The handlers' `Db` writes hit the backend database — initialize
        // it on the shared ease runtime (a throwaway dir per run).
        let db_dir = std::env::temp_dir().join(format!(
            "ease-emp-rpc-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&db_dir).expect("temp db dir");
        ease_client_tokio::tokio_runtime()
            .block_on(cx.database_server().init(db_dir.to_string_lossy().to_string()))
            .expect("db init");

        let bus = RpcBus::new();
        let (tx, rx) = mpsc::channel::<Cmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<(u64, ())>();
        let bus_for_plugin = Some(bus.clone());
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
                .plugin(TurRpcBusPlugin::new(bus_for_plugin))
                .plugin(EaseMusicPlugin::new(cx))
                .build()
                .expect("runtime build");
            let handle = PluginHandle::mint(
                PluginId::new("com.ease.emp-rpc-fixture"),
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
            futures::executor::block_on(app.load_rut_module_cx(FIXTURE, boot_cx))
                .expect("the fixture must compile and boot");
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
                    Cmd::Token(rtx) => {
                        let _ = rtx.send(futures::executor::block_on(app.rut_start_answer()));
                    }
                }
            }
        });
        let (boot_cx, ()) = ready_rx.recv().expect("driver thread booted");
        RpcClient::new(bus, Arc::new(TestDriver { tx }), boot_cx)
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

    const PAYLOAD: &[u8] = b"hello world!";

    #[test]
    fn the_plan_shape_fixture_boots_and_serves_str_ops() {
        let client = boot();
        let reply = runtime()
            .block_on(client.call_host(
                "storage:list",
                serde_json::json!({ "pluginId": "com.ease.emp-rpc-fixture", "dir": "/music" }),
            ))
            .expect("the bound handler answers");
        assert_eq!(
            reply,
            serde_json::json!([
                { "name": "song.flac", "path": "/music/song.flac", "size": 12, "isDir": false }
            ]),
            "the emp-class-bound handler decoded its own args and answered the raw payload"
        );
    }

    #[test]
    fn bytes_ops_ride_the_binary_lane_raw() {
        let client = boot();
        let payload: Vec<u8> = PAYLOAD.iter().copied().collect();
        let reply = runtime()
            .block_on(client.call_host_bytes("lyric:parse", payload.clone()))
            .expect("dispatch ok")
            .expect("the handler answered");
        assert_eq!(reply, payload, "raw bytes through, no base64 anywhere");

        // nil = "not mine".
        let none = runtime()
            .block_on(client.call_host_bytes("lyric:parse:other", b"x".to_vec()))
            .expect("dispatch ok");
        assert!(none.is_none(), "the nil answer is the not-mine arm");
    }

    #[test]
    fn a_handler_trap_keeps_the_prefix_law() {
        let client = boot();
        let err = runtime()
            .block_on(client.call_host("fail:unauthorized", serde_json::json!({})))
            .expect_err("the trap propagates as the error arm");
        match err {
            RpcError::Handler(msg) => {
                assert_eq!(msg, "UNAUTHORIZED: token expired");
            }
            other => panic!("expected Handler, got {other:?}"),
        }
    }

    #[test]
    fn events_fan_out_through_the_bind_table() {
        let client = boot();
        runtime().block_on(client.emit_event(
            "music:play",
            serde_json::json!({ "musicId": "42" }),
        ));
        // The subscriber recorded the payload through its `Db` handle.
        let reply = runtime()
            .block_on(client.call_host("test:last-event", serde_json::json!({})))
            .expect("dispatch ok");
        assert_eq!(reply, serde_json::json!({ "musicId": "42" }));
    }

    /// The full storage contract — opener + chunked pull + release — over
    /// the bind-table lane, through `JsStorageBackend` unchanged.
    #[test]
    fn the_storage_backend_pulls_chunks_over_the_new_lane() {
        let client = boot();
        let tokio_rt = runtime();
        let backend = JsStorageBackend::new(
            client,
            "com.ease.emp-rpc-fixture",
            "fixture:test",
            tokio_rt.handle().clone(),
        );
        let entries = tokio_rt.block_on(backend.list("/music".into())).expect("list");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "song.flac");
        assert_eq!(entries[0].size, Some(PAYLOAD.len()));

        let file = tokio_rt
            .block_on(backend.get("/music/song.flac".into(), 0))
            .expect("get");
        assert_eq!(file.name(), "song.flac");
        assert_eq!(file.total_size(), Some(PAYLOAD.len()));
        let bytes = tokio_rt.block_on(file.bytes()).expect("bytes");
        assert_eq!(bytes.as_ref(), PAYLOAD);
    }
}
