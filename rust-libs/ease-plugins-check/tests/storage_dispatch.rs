//! Dispatch-gate for the three storage backends: drive a REAL
//! `storage:list` request through the host's own lane (`RpcClient` → the
//! bind-table dispatch entry) into each backend and require the failure
//! to be past **arg-decode** — i.e. the handler must not trap the
//! "bad storage:list request" decode error.
//!
//! This is the hole the third review fell through: `enter_args` returned
//! with the reader cursor ON the args object's `{`, so every field loop
//! died sticky and EVERY storage op in all three providers answered the
//! decode error — browse, streaming playback and OAuth all dead — while
//! compile+boot and the contract-registration checks stayed green. (The
//! phase-4b emp port turned each op's decode failure into a trap, so the
//! gates read `RpcError::Handler` messages now.)

#[cfg(test)]
mod storage_dispatch {
    use ease_client_android::plugin_runtime::emp::PluginHandle;
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId, PluginInstance};
    use ease_client_backend::ctx::BackendContext;
    use ease_client_backend::services::plugin_pkg::{read_entry_source, EntrySource};
    use ease_tur_rpc::{DispatchAnswer, RpcBus, RpcClient, RpcError, RutDriver, TurRpcBusPlugin};
    use std::path::Path;
    use std::sync::mpsc;
    use std::sync::Arc;
    use tur_engine::core::rut_runtime::RutModuleFiles;
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::{TurRuntime, TurStdPlugin};
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
        Token(mpsc::SyncSender<u64>),
    }

    struct TestDriver {
        tx: mpsc::Sender<Cmd>,
    }

    impl RutDriver for TestDriver {
        fn call_entry_str(&self, _name: &str, _cx: u64) -> Result<String, String> {
            Err("the storage backends serve no stash-bus entries".to_string())
        }

        fn call_entry(&self, _name: &str, _cx: u64) -> Result<(), String> {
            Err("the storage backends serve no stash-bus entries".to_string())
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

    /// Boot `plugin_dir`'s backend (stamped with a minted `PluginHandle`
    /// for `plugin_id`) and return the host client — the same
    /// production headless shape the device's `spawn_headless` runs
    /// (handle + `RpcIdentity` stamps, cx-bearing module load).
    fn boot(plugin_id: &str, plugin_dir: &str) -> RpcClient {
        // The entrypoint resolves through the manifest's `backend`
        // spelling via the scan's own lane (`read_entry_source`) — the
        // migrated plugins are pkg trees; no flat `backend.rut` exists.
        let dir = Path::new(plugin_dir);
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("manifest.json")).expect("manifest.json readable"),
        )
        .expect("manifest.json parses");
        let backend = manifest["backend"].as_str().expect("manifest backend field");
        let source = read_entry_source(dir, backend).expect("backend entrypoint readable");

        let cx = BackendContext::new();
        // The config lookups past arg-decode hit `ease.db` — initialize the
        // backend database (on the shared ease runtime, where the rows'
        // block_on lives) or the op panics on the lane.
        let db_dir = std::env::temp_dir().join(format!(
            "ease-storage-dispatch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&db_dir).expect("temp db dir");
        ease_client_tokio::tokio_runtime().block_on(
            cx.database_server().init(db_dir.to_string_lossy().to_string()),
        ).expect("db init");
        let bus = RpcBus::new();
        let (tx, rx) = mpsc::channel::<Cmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<u64>();
        let bus_for_plugin = Some(bus.clone());
        let pid = plugin_id.to_string();
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
            let handle = PluginHandle::mint(PluginId::new(pid), PluginInstance(None));
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
            loaded.expect("backend loads");
            ready_tx.send(boot_cx).expect("test still waiting");
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
        let boot_cx = ready_rx.recv().expect("driver thread booted");
        RpcClient::new(bus, Arc::new(TestDriver { tx }), boot_cx)
    }

    /// One host-shaped op call through the bind-table dispatch entry,
    /// answering the handler's trap message. Every gate here drives an
    /// op that MUST fail in the harness (no config / no session) — an
    /// answer is unexpected.
    fn dispatch(client: &RpcClient, op: &str, args: &str) -> String {
        let args: serde_json::Value = serde_json::from_str(args).expect("args json parses");
        match runtime().block_on(client.call_host(op, args)) {
            Ok(v) => panic!("{op}: expected a handler trap, got {v}"),
            Err(RpcError::Handler(msg)) => msg,
            Err(other) => panic!("{op}: unexpected rpc error: {other}"),
        }
    }

    /// Each storage backend: a real `storage:list` request must get PAST
    /// arg-decode. Without config the op can't fully succeed — but the
    /// failure must be the config-miss (or auth) layer, never the
    /// "bad storage:list request" decode error.
    #[test]
    fn storage_list_requests_decode_in_every_backend() {
        let cases = [
            (
                "com.ease.webdav",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.webdav"),
            ),
            (
                "com.ease.onedrive",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.onedrive"),
            ),
            (
                "com.ease.s3",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.s3"),
            ),
        ];
        for (id, dir) in cases {
            let client = boot(id, dir);
            let request = format!(
                "{{\"pluginId\":\"{id}\",\"storageId\":\"{id}:test\",\"dir\":\"/\"}}"
            );
            let msg = dispatch(&client, "storage:list", &request);
            assert!(
                !msg.contains("bad storage:list request"),
                "{id}: the args decode died sticky — {msg}"
            );
        }
    }

    /// The opener op too — the one streaming playback actually rides
    /// first (`{"pluginId","storageId","path","offset"}` from
    /// ease-js-storage's `get`); its decode loop is the on-device hole
    /// the other gates missed.
    #[test]
    fn storage_get_opener_requests_decode() {
        let cases = [
            (
                "com.ease.webdav",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.webdav"),
            ),
            (
                "com.ease.onedrive",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.onedrive"),
            ),
            (
                "com.ease.s3",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.s3"),
            ),
        ];
        for (id, dir) in cases {
            let client = boot(id, dir);
            // The exact ease-js-storage shape — including a non-ASCII,
            // percent-encoded path like the restored device data carries.
            let request = format!(
                "{{\"pluginId\":\"{id}\",\"storageId\":\"{id}:186df89d-ac7f-490f-9ac1-2653c3172465\",\"path\":\"/data/musics/Youtube%202026/%E3%80%90%E6%AD%8C%E3%81%A3%E3%81%A6%E3%81%BF%E3%81%9F%E3%80%91test%20track.mp3\",\"offset\":0}}"
            );
            let msg = dispatch(&client, "storage:get", &request);
            assert!(
                !msg.contains("bad storage:get request"),
                "{id}: the storage:get args decode died sticky — {msg}"
            );
            // Past decode: the failure (no config in the harness db) must
            // be the config-miss layer.
            assert!(
                msg.contains("no config"),
                "{id}: expected the config-miss layer past decode, got: {msg}"
            );
        }
    }

    /// Finding-4 gate: a scheme-less endpoint (`s3.amazonaws.com`) must
    /// normalize to https (the TS `normalizeEndpoint` default) — the connect
    /// op must NOT answer "invalid endpoint" for it. (Missing credentials
    /// fail later, with a different message.)
    #[test]
    fn s3_scheme_less_endpoints_default_to_https() {
        let client = boot(
            "com.ease.s3",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.s3"),
        );
        let request = "{\"endpoint\":\"s3.amazonaws.com\",\"region\":\"us-east-1\",\"bucket\":\"b\",\"alias\":\"t\",\"accessKeyId\":\"k\",\"secretAccessKey\":\"s\"}";
        let msg = dispatch(&client, "s3:connect", request);
        assert!(
            !msg.contains("invalid endpoint"),
            "scheme-less endpoints must default to https — got: {msg}"
        );
    }

    /// The chunk-pull op too (its arg loop is the one the streaming path
    /// rides): decode must not die. All three providers — onedrive's
    /// streaming path in particular was never runtime-verified until this
    /// gate existed.
    #[test]
    fn storage_get_chunk_requests_decode() {
        let cases = [
            (
                "com.ease.webdav",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.webdav"),
            ),
            (
                "com.ease.onedrive",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.onedrive"),
            ),
            (
                "com.ease.s3",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.s3"),
            ),
        ];
        for (id, dir) in cases {
            let client = boot(id, dir);
            let request = "{\"sid\":1,\"offset\":0,\"len\":262144}";
            let msg = dispatch(&client, "storage:get-chunk", request);
            assert!(
                !msg.contains("bad storage:get-chunk request"),
                "{id}: the args decode died sticky — {msg}"
            );
        }
    }
}
