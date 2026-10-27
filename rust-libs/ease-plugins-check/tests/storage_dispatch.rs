//! Dispatch-gate for the three storage backends: drive a REAL
//! `storage:list` request (`{"op":…,"args":{…}}` through the bus stash)
//! into each backend and require the reply to be past **arg-decode** —
//! i.e. the reply must not be the "bad storage:list request" decode error.
//!
//! This is the hole the third review fell through: `enter_args` returned
//! with the reader cursor ON the args object's `{`, so every field loop
//! died sticky and EVERY storage op in all three providers answered the
//! decode error — browse, streaming playback and OAuth all dead — while
//! compile+boot and the contract-registration checks stayed green.

#[cfg(test)]
mod storage_dispatch {
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId};
    use ease_client_backend::ctx::BackendContext;
    use ease_tur_rpc::{RpcBus, TurRpcBusPlugin};
    use std::sync::mpsc;
    use std::sync::Arc;
    use tur_engine::core::plugin::Plugin;
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::{TurRuntime, TurStdPlugin};
    use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
    use tur_native::NativeFontLoader;

    enum Cmd {
        CallStr(String, u64, mpsc::SyncSender<Result<String, String>>),
        Token(mpsc::SyncSender<u64>),
    }

    /// Boot `plugin_dir`'s backend (stamped with `plugin_id`) and return
    /// the bus + the driver channel.
    fn boot(plugin_id: &str, plugin_dir: &str) -> (Arc<RpcBus>, mpsc::Sender<Cmd>) {
        let source =
            std::fs::read_to_string(plugin_dir.to_string() + "/backend.rut")
                .expect("backend.rut readable");

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
        let (ready_tx, ready_rx) = mpsc::channel::<()>();
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
            let (app, _looper) = runtime
                .app_builder()
                .worker_pool(pool)
                .instance_data(|icx| {
                    icx.define::<PluginId>(PluginId::new(pid));
                })
                .build_headless((0.0, 0.0))
                .expect("headless app build");
            futures::executor::block_on(app.load_rut_module(source.as_str()))
                .expect("backend loads");
            ready_tx.send(()).expect("test still waiting");
            for cmd in rx {
                match cmd {
                    Cmd::CallStr(name, cx, rtx) => {
                        let _ = rtx.send(
                            futures::executor::block_on(app.call_rut_entry_cx_str(&name, cx))
                                .map_err(|e| e.to_string()),
                        );
                    }
                    Cmd::Token(rtx) => {
                        let _ = rtx.send(futures::executor::block_on(app.rut_start_answer()));
                    }
                }
            }
        });
        ready_rx.recv().expect("driver thread booted");
        (bus, tx)
    }

    /// Stash the request and fire the named export; answer the reply string.
    fn dispatch(bus: &RpcBus, tx: &mpsc::Sender<Cmd>, export: &str, request: &str) -> String {
        bus.stash_request(request.to_string());
        let (token_tx, token_rx) = mpsc::sync_channel(1);
        tx.send(Cmd::Token(token_tx)).ok();
        let token = token_rx.recv().expect("token");
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        tx.send(Cmd::CallStr(export.into(), token, reply_tx))
            .expect("driver alive");
        reply_rx
            .recv()
            .expect("reply")
            .expect("the export must not trap on a well-formed request")
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
                "op_storage_list",
            ),
            (
                "com.ease.onedrive",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.onedrive"),
                "op_storage_list",
            ),
            (
                "com.ease.s3",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.s3"),
                "op_storage_list",
            ),
        ];
        for (id, dir, export) in cases {
            let (bus, tx) = boot(id, dir);
            let request = format!(
                "{{\"op\":\"storage:list\",\"args\":{{\"pluginId\":\"{id}\",\"storageId\":\"{id}:test\",\"dir\":\"/\"}}}}"
            );
            let reply = dispatch(&bus, &tx, export, &request);
            assert!(
                !reply.contains("bad storage:list request"),
                "{id}: the args decode died sticky — {reply}"
            );
        }
    }

    /// Finding-4 gate: a scheme-less endpoint (`s3.amazonaws.com`) must
    /// normalize to https (the TS `normalizeEndpoint` default) — the connect
    /// op must NOT answer "invalid endpoint" for it. (Missing credentials
    /// fail later, with a different message.)
    #[test]
    fn s3_scheme_less_endpoints_default_to_https() {
        let (bus, tx) = boot(
            "com.ease.s3",
            concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.s3"),
        );
        let request = "{\"op\":\"s3:connect\",\"args\":{\"endpoint\":\"s3.amazonaws.com\",\"region\":\"us-east-1\",\"bucket\":\"b\",\"alias\":\"t\",\"accessKeyId\":\"k\",\"secretAccessKey\":\"s\"}}";
        let reply = dispatch(&bus, &tx, "op_s3_connect", request);
        assert!(
            !reply.contains("invalid endpoint"),
            "scheme-less endpoints must default to https — got: {reply}"
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
                "op_storage_get_chunk",
            ),
            (
                "com.ease.onedrive",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.onedrive"),
                "op_storage_get_chunk",
            ),
            (
                "com.ease.s3",
                concat!(env!("CARGO_MANIFEST_DIR"), "/../../plugins/com.ease.s3"),
                "op_storage_get_chunk",
            ),
        ];
        for (id, dir, export) in cases {
            let (bus, tx) = boot(id, dir);
            let request = format!(
                "{{\"op\":\"storage:get-chunk\",\"args\":{{\"sid\":1,\"offset\":0,\"len\":262144}}}}"
            );
            let reply = dispatch(&bus, &tx, export, &request);
            assert!(
                !reply.contains("bad storage:get-chunk request"),
                "{id}: the args decode died sticky — {reply}"
            );
        }
    }
}
