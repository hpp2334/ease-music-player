//! The lyric bytes seam, end-to-end: boot the REAL `com.ease.lyricformats`
//! backend and drive `lyric:parse` through the RPC binary lane exactly as
//! the host dispatch does (`RpcClient::call_host_bytes`, the
//! `<json header>\n<content bytes>` framing), then map the reply through
//! the host's own result shape.
//!
//! This is the phase-5 gate for the framing contract: the plugin decodes
//! its own args (raw bytes — no base64 anywhere), the header split rides
//! the FIRST newline, a foreign `parserId` answers the nil "not mine" arm,
//! and a malformed header traps.

#[cfg(test)]
mod lyric_bytes_lane {
    use ease_client_android::plugin_runtime::emp::PluginHandle;
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId, PluginInstance};
    use ease_client_backend::ctx::BackendContext;
    use ease_client_backend::services::plugin_pkg::{read_entry_source, EntrySource};
    use ease_tur_rpc::{DispatchAnswer, RpcClient, RpcError, RutDriver, TurRpcBindPlugin};
    use std::path::Path;
    use std::sync::mpsc;
    use std::sync::Arc;
    use tur_engine::core::rut_runtime::RutModuleFiles;
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::{TurRuntime, TurStdPlugin};
    use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
    use tur_native::NativeFontLoader;

    enum Cmd {
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
            _name: &str,
            _cx: u64,
            _op: &str,
            _args: &str,
        ) -> Result<DispatchAnswer, String> {
            Err("the lyric backend serves no str ops".to_string())
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

    /// Boot the real lyricformats pkg tree (stamped with a minted
    /// `PluginHandle` for the plugin id — the production headless shape).
    fn boot() -> RpcClient {
        let dir = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../plugins/com.ease.lyricformats"
        ));
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join("manifest.json")).expect("manifest.json readable"),
        )
        .expect("manifest.json parses");
        let backend = manifest["backend"].as_str().expect("manifest backend field");
        let source = read_entry_source(dir, backend).expect("backend entrypoint readable");

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
            let handle = PluginHandle::mint(
                PluginId::new("com.ease.lyricformats"),
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
            loaded.expect("lyricformats backend loads");
            ready_tx.send(boot_cx).expect("test still waiting");
            for cmd in rx {
                match cmd {
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
        let boot_cx = ready_rx.recv().expect("driver thread booted");
        RpcClient::new(Arc::new(TestDriver { tx }), boot_cx)
    }

    /// The host's exact framing: compact JSON header, ONE newline, then
    /// the raw content bytes.
    fn frame(parser_id: &str, file_name: &str, content: &[u8]) -> Vec<u8> {
        let mut out = serde_json::to_vec(&serde_json::json!({
            "parserId": parser_id,
            "fileName": file_name,
        }))
        .expect("header serializes");
        out.push(b'\n');
        out.extend_from_slice(content);
        out
    }

    /// The host's reply mapping (services/lyrics `map_plugin_result`):
    /// permissive, `None` = fall through.
    fn map_reply(reply: &[u8]) -> Option<Vec<(u64, String)>> {
        let value: serde_json::Value = serde_json::from_slice(reply).ok()?;
        let lines = value.get("lines")?.as_array()?;
        if lines.is_empty() {
            return None;
        }
        let mut out = Vec::new();
        for line in lines {
            let ms = line.get("timeMs")?.as_f64()?;
            if !ms.is_finite() {
                return None;
            }
            let text = line.get("text")?.as_str()?.to_string();
            out.push((ms.round().clamp(0.0, u64::MAX as f64) as u64, text));
        }
        if out.is_empty() {
            return None;
        }
        out.sort_by_key(|(ms, _)| *ms);
        Some(out)
    }

    /// A multi-byte UTF-8 payload with a non-ASCII fileName (the header
    /// stays one line; the content is arbitrary text).
    const LRC: &str = "[ti:测试]\n[offset:+500]\n[00:01.50]first\n[00:06.25]second\n";

    #[test]
    fn the_framed_bytes_lane_parses_lrc() {
        let client = boot();
        let payload = frame("lrc", "/music/歌 - test.lrc", LRC.as_bytes());
        let reply = runtime()
            .block_on(client.call_host_bytes("lyric:parse", payload))
            .expect("dispatch ok")
            .expect("the parser answered the framed payload");
        let lines = map_reply(&reply).expect("usable lines");
        assert_eq!(
            lines,
            vec![(1500, "first".into()), (6250, "second".into())],
            "timed lines out of the framed bytes, no base64 anywhere"
        );
    }

    #[test]
    fn an_unframed_payload_is_a_handler_trap() {
        let client = boot();
        let err = runtime()
            .block_on(client.call_host_bytes("lyric:parse", b"no newline here".to_vec()))
            .expect_err("unframed payload");
        match err {
            RpcError::Handler(msg) => {
                assert!(
                    msg.contains("unframed payload"),
                    "the trap must name the framing law, got: {msg}"
                );
            }
            other => panic!("expected Handler, got {other:?}"),
        }
    }

    #[test]
    fn an_unknown_parser_answers_not_mine() {
        let client = boot();
        let payload = frame("krc", "/music/song.krc", LRC.as_bytes());
        let reply = runtime()
            .block_on(client.call_host_bytes("lyric:parse", payload))
            .expect("dispatch ok");
        assert!(reply.is_none(), "the nil answer is the not-mine arm");
    }
}
