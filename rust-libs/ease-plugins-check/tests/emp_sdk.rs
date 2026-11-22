//! The phase-2 `emp` SDK check: a backend-shaped fixture constructs
//! every `emp` service from the boot handle (`start(cx)`) and drives
//! REAL `emp_host` rows through them — against the full app plugin set
//! (the `emp` + 3rd-lib offerings ride `EaseMusicPlugin`), a temp
//! backend database, and the phase-1 `start_cx` lane.
//!
//! Gates:
//! 1. the fixture compiles AND boots with `load_rut_module_cx` (the
//!    `emp_host` surface resolves, the boot law holds — constructors
//!    store the handle, `db.set` at start runs a real row),
//! 2. the round-trip probe (addressed by the boot identity) answers
//!    through Db / OAuth / Secret — a KV set/get, a flow mint, a
//!    secret put/get,
//! 3. the foreign-opaque gate: `Db(opaque(12345))` TRAPS at the row
//!    with the `emp:db:` message — script-minted boxes never decode.
//!
//! Companion to `emp_grammar.rs` (the grammar spike — the class shapes
//! without services) and `storage_dispatch.rs` (the harness shape this
//! reuses).

#[cfg(test)]
mod emp_sdk {
    use ease_client_android::plugin_runtime::emp::PluginHandle;
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId, PluginInstance};
    use ease_client_backend::ctx::BackendContext;
    use ease_tur_rpc::TurRpcBusPlugin;
    use std::sync::mpsc;
    use std::sync::Arc;
    use tur_engine::core::plugin::Plugin;
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::{TurStdPlugin, TurRuntime};
    use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
    use tur_native::NativeFontLoader;

    const FIXTURE: &str = include_str!("emp_sdk_fixture.rut");

    enum Cmd {
        /// `entry fn(opaque) -> str` — addressed by the boot identity.
        CallStr(String, u64, mpsc::SyncSender<Result<String, String>>),
    }

    /// Boot the fixture (stamped with a minted `PluginHandle`, loaded
    /// through the phase-1 cx lane) and return the driver channel.
    fn boot() -> (u64, mpsc::Sender<Cmd>) {
        let cx = BackendContext::new();
        // The rows' block_on lives on the shared ease runtime — initialize
        // the backend database (a throwaway dir per run).
        let db_dir = std::env::temp_dir().join(format!(
            "ease-emp-sdk-{}-{}",
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
                .plugin(TurRpcBusPlugin::new(None))
                .plugin(EaseMusicPlugin::new(cx))
                .build()
                .expect("runtime build");
            // The host-minted handle, stamped + delivered through the
            // phase-1 lane — the production headless path's exact shape.
            let handle = PluginHandle::mint(
                PluginId::new("com.ease.emp-sdk-fixture"),
                PluginInstance(None),
            );
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

    fn probe(tx: &mpsc::Sender<Cmd>, name: &str, cx: u64) -> Result<String, String> {
        let (reply_tx, reply_rx) = mpsc::sync_channel(1);
        tx.send(Cmd::CallStr(name.into(), cx, reply_tx))
            .expect("driver alive");
        reply_rx.recv().expect("reply")
    }

    #[test]
    fn emp_services_round_trip_through_the_rows() {
        let (boot_cx, tx) = boot();
        let reply = probe(&tx, "op_roundtrip", boot_cx).expect("the probe must not trap");
        let parts: Vec<&str> = reply.split('|').collect();
        assert_eq!(parts.len(), 4, "the round-trip answers all four lanes: {reply}");
        assert_eq!(parts[0], "hola", "the KV set/get through Db round-trips");
        assert!(
            parts[1].starts_with("oauth-"),
            "the OAuth flow mint went through the handle: {}",
            parts[1]
        );
        assert_eq!(parts[2], "s3cret", "the secret put/get round-trips");
        assert_eq!(parts[3], "ok", "the boot-time KV write persisted");
    }

    #[test]
    fn a_script_minted_handle_traps_at_the_row() {
        // The lane needs A live slot (the boot identity); the forged
        // opaque is built INSIDE the fixture — the trap fires at the row.
        let (boot_cx, tx) = boot();
        let reply = probe(&tx, "op_forge", boot_cx).expect_err("the forged handle must trap");
        assert!(
            reply.contains("emp:db:"),
            "the trap must name the emp namespace, got: {reply}"
        );
        assert!(
            reply.contains("foreign handle opaque"),
            "the trap must name the foreign-opaque law, got: {reply}"
        );
    }
}
