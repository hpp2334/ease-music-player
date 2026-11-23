//! The 3rd-party util lib check (`plugins/3rd/`): the lib rut sources
//! compile against the offered set and their host rows bind and RUN.
//! Offered pkgs only compile when used, so without this fixture the
//! lib sources (and the pure-rut time formatters) would never be
//! exercised — the shipped plugins still use the old `tur_host::ease`
//! rows (D5).
//!
//! Gated with known-good values: MD5("abc"),
//! `http_date(0)` / `amz_date(0)` (the pure-rut formatters against the
//! epoch), base64("hello"), the XML tree shape, and the net sync
//! one-shot's transport-failure envelope (a closed port — status 0).

#[cfg(test)]
mod third_libs {
    use ease_client_android::plugin_runtime::emp::PluginHandle;
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId, PluginInstance};
    use ease_client_backend::ctx::BackendContext;
    use ease_tur_rpc::TurRpcBindPlugin;
    use std::sync::mpsc;
    use std::sync::Arc;
    
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::{TurRuntime, TurStdPlugin};
    use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
    use tur_native::NativeFontLoader;

    const FIXTURE: &str = include_str!("third_libs_fixture.rut");

    enum Cmd {
        CallStr(String, u64, mpsc::SyncSender<Result<String, String>>),
    }

    fn boot() -> (u64, mpsc::Sender<Cmd>) {
        let cx = BackendContext::new();
        let (tx, rx) = mpsc::channel::<Cmd>();
        let (ready_tx, ready_rx) = mpsc::channel::<u64>();
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
            let handle =
                PluginHandle::mint(PluginId::new("com.ease.third-libs"), PluginInstance(None));
            let boot_cx = handle.id();
            let (app, _looper) = runtime
                .app_builder()
                .worker_pool(pool)
                .instance_data(|icx| {
                    icx.define::<PluginHandle>(handle);
                })
                .build_headless((0.0, 0.0))
                .expect("headless app build");
            futures::executor::block_on(app.load_rut_module_cx(FIXTURE, boot_cx))
                .expect("the fixture must compile and boot");
            ready_tx.send(boot_cx).expect("test still waiting");
            for cmd in rx {
                match cmd {
                    Cmd::CallStr(name, cx, rtx) => {
                        let _ = rtx.send(
                            futures::executor::block_on(app.call_rut_entry_cx_str(&name, cx))
                                .map_err(|e| e.to_string()),
                        );
                    }
                }
            }
        });
        let boot_cx = ready_rx.recv().expect("driver thread booted");
        (boot_cx, tx)
    }

    #[test]
    fn third_libs_compile_and_answer_known_values() {
        let (boot_cx, tx) = boot();
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        tx.send(Cmd::CallStr("probe_libs".into(), boot_cx, reply_tx))
            .expect("driver alive");
        let reply = reply_rx
            .recv()
            .expect("reply")
            .expect("the lib probe must not trap");

        let parts: Vec<&str> = reply.split('|').collect();
        assert_eq!(parts.len(), 6, "the probe answers all six lanes: {reply}");
        // crypto: MD5("abc") + base64("hello") — the host rows bound.
        assert_eq!(parts[0], "900150983cd24fb0d6963f7d28e17f72", "md5");
        assert_eq!(parts[1], "aGVsbG8=", "b64_encode");
        // time: the PURE-RUT formatters against the epoch.
        assert_eq!(parts[2], "Thu, 01 Jan 1970 00:00:00 GMT", "http_date(0)");
        assert_eq!(parts[3], "19700101T000000Z", "amz_date(0)");
        // xml: the JSON tree shape (attrs/text/children).
        assert_eq!(
            parts[4],
            r#"{"children":{"a":{"children":{"b":{"text":"1"}}}}}"#,
            "xml parse"
        );
        // net: the sync one-shot against a closed port — the transport
        // failure envelope (status 0, error set), never a trap.
        let net: serde_json::Value =
            serde_json::from_str(parts[5]).expect("the net envelope is JSON");
        assert_eq!(net["status"], 0, "closed port answers transport failure");
        assert!(
            net["error"].as_str().map(|e| !e.is_empty()).unwrap_or(false),
            "the error lane names the failure: {net}"
        );
    }
}
