//! Compile + boot validation for the shipped rut plugin sources.
//!
//! Every plugin under `plugins/<id>/` must compile and boot on the host
//! harness: the `backend` entrypoint loads with the FULL app plugin set
//! (`EaseMusicPlugin` included — its rows register, though Android-only
//! upcalls are unreachable and must not fire at boot) and every
//! contribution `view` loads into a fresh instance (views never call
//! rows at boot — they build UI in `start`). Entrypoints resolve through
//! the scan's own lane ([`ease_client_backend::services::plugin_pkg`]),
//! so single-file sources and pkg-tree directories (`mod.rut` +
//! `mod NAME;` children) validate identically to how the device scan
//! registers them.
//!
//! This is the plugin toolchain: there is no rspack/pnpm build anymore —
//! the engine compiles the sources, and this suite is what `pnpm test`
//! runs to catch a broken plugin before it ever reaches a device.

// This crate is a pure test harness — all logic lives behind cfg(test) so
// the (empty) lib target builds without the dev-dependencies.
#[cfg(test)]
mod plugin_checks {
    use std::path::{Path, PathBuf};

    use ease_client_android::plugin_runtime::emp::PluginHandle;
    use ease_client_android::plugin_runtime::{EaseMusicPlugin, PluginId, PluginInstance};
    use ease_client_backend::ctx::BackendContext;
    use ease_client_backend::services::plugin_pkg::{read_entry_source, EntrySource};
    use ease_tur_rpc::TurRpcBindPlugin;
    use std::sync::Arc;
    
    use tur_engine::core::rut_runtime::RutModuleFiles;
    use tur_engine::core::scheduler::WorkerPoolHandle;
    use tur_engine::{TurRuntime, TurStdPlugin};
    use tur_integration_tests::{MutexFixedClock, TestSchedulerDriver};
    use tur_native::NativeFontLoader;

    fn plugins_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins")
            .canonicalize()
            .expect("plugins dir")
    }

    fn manifest_of(dir: &Path) -> serde_json::Value {
        let raw = std::fs::read_to_string(dir.join("manifest.json"))
            .expect("manifest.json readable");
        serde_json::from_str(&raw).expect("manifest.json parses")
    }

    /// Load one resolved entrypoint into a fresh harness instance; panic
    /// with the compile diagnostics on failure (rut compile errors name
    /// the row + line). A single file rides the single rail, a pkg-tree
    /// file set the engine's multi-file rail — the exact two shapes a
    /// scan can register (the caller picks via the shared
    /// `read_entry_source` lane, so the check exercises the same
    /// resolution law the device scan runs).
    ///
    /// The instance is built in the production headless shape (the same
    /// recipe the device's `spawn_headless` runs and the sibling
    /// integration tests mirror): stamped with a minted `PluginHandle`
    /// (+ its `RpcIdentity` view) and loaded through the cx lane with
    /// that id. The migrated plugins' `entry fn start(cx)` refuses to
    /// boot without it — the emp services' decode law verifies the
    /// sealed handle against the instance's own stamp, and `TurTestApp`
    /// has no instance-data hook, so the raw builder it is.
    fn assert_loads_source(source: EntrySource, what: String) {
        let cx = BackendContext::new();
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
                // The rpc bind-table rows + the sealed-handler table (the
                // stamps `emp_host`'s bodies resolve).
                .plugin(TurRpcBindPlugin::new())
                .plugin(EaseMusicPlugin::new(cx))
                .build()
                .expect("runtime build");
            let handle = PluginHandle::mint(
                PluginId::new("com.ease.harness"),
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
                .build_headless((400.0, 400.0))
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
            if let Err(e) = loaded {
                panic!("{what} failed to compile/boot: {e}");
            }
        })
        .join()
        .unwrap_or_else(|e| std::panic::resume_unwind(e));
    }

    /// Load one entrypoint reference (a manifest `backend` / `view`
    /// value — single file OR pkg-tree directory).
    fn assert_loads(dir: &Path, entry: &str, what: String) {
        let source = read_entry_source(dir, entry)
            .unwrap_or_else(|e| panic!("{what}: entrypoint unreadable: {e}"));
        assert_loads_source(source, what);
    }

    /// Every text file of one entrypoint (the root, then the declared
    /// mod children) — the contract-op probes read the whole set, since
    /// a pkg-tree backend may register its ops from a child file.
    fn entry_texts(dir: &Path, entry: &str) -> Vec<String> {
        match read_entry_source(dir, entry) {
            Ok(EntrySource::Single(src)) => vec![src],
            Ok(EntrySource::FileSet { root, mods }) => {
                let mut out = vec![root];
                out.extend(mods.into_values());
                out
            }
            Err(e) => panic!("{entry}: entrypoint unreadable: {e}"),
        }
    }

    fn plugin_dirs() -> Vec<PathBuf> {
        let root = plugins_root();
        std::fs::read_dir(&root)
            .expect("plugins dir readable")
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|dir| {
                dir.is_dir()
                    && !dir.ends_with("infra")
                    && !dir.ends_with("registry")
                    && dir.join("manifest.json").is_file()
            })
            .collect()
    }

    #[test]
    fn every_plugin_compiles_and_boots() {
        let mut checked = 0usize;
        for dir in plugin_dirs() {
            let manifest = manifest_of(&dir);
            let id = manifest["id"].as_str().expect("manifest id").to_string();
            let api = manifest["apiVersion"].as_i64().unwrap_or(0);
            assert_eq!(api, 3, "{id}: manifest apiVersion must be 3 (the emp epoch)");

            if let Some(backend) = manifest["backend"].as_str() {
                assert_loads(&dir, backend, format!("{id}/{backend}"));
                checked += 1;
            }
            let views: Vec<&str> = ["storages", "dashboard"]
                .iter()
                .filter_map(|kind| manifest["contributions"][kind].as_array())
                .flat_map(|arr| arr.iter())
                .filter_map(|c| c["view"].as_str())
                .collect();
            for view in views {
                assert_loads(&dir, view, format!("{id}/{view}"));
                checked += 1;
            }
        }
        assert!(!plugin_dirs().is_empty(), "no plugins found");
        assert!(checked > 0, "no rut sources checked — the plugins are gone?");
    }

    /// The contract ops every storage backend must BIND (the literals
    /// `ease-js-storage` dispatches, through the bind table —
    /// `rpc.bind("op", …)` in the rut sources). A backend with `storages`
    /// contributions must bind all four.
    #[test]
    fn storage_backends_register_the_contract_ops() {
        for dir in plugin_dirs() {
            let manifest = manifest_of(&dir);
            let id = manifest["id"].as_str().expect("manifest id").to_string();
            let has_storages = manifest["contributions"]["storages"]
                .as_array()
                .is_some_and(|a| !a.is_empty());
            let Some(backend) = manifest["backend"].as_str() else {
                continue;
            };
            if !has_storages {
                continue;
            }
            let texts = entry_texts(&dir, backend);
            for op in [
                "storage:list",
                "storage:get",
                "storage:get-chunk",
                "storage:release",
            ] {
                assert!(
                    texts.iter().any(|t| t.contains(&format!("bind(\"{op}\""))),
                    "{id}: storage backend must bind the contract op `{op}` (rpc.bind)"
                );
            }
        }
    }

    /// Lyric-parser backends must bind `lyric:parse` on the BINARY lane
    /// (`rpc.bind_bin` — the host's lyric seam ships raw bytes through the
    /// `<json header>\n<content>` framing).
    #[test]
    fn lyric_parsers_register_the_parse_op() {
        for dir in plugin_dirs() {
            let manifest = manifest_of(&dir);
            let id = manifest["id"].as_str().expect("manifest id").to_string();
            let has_parsers = manifest["contributions"]["lyricParsers"]
                .as_array()
                .is_some_and(|a| !a.is_empty());
            let Some(backend) = manifest["backend"].as_str() else {
                continue;
            };
            if !has_parsers {
                continue;
            }
            let texts = entry_texts(&dir, backend);
            assert!(
                texts.iter().any(|t| t.contains("bind_bin(\"lyric:parse\"")),
                "{id}: lyric parser backend must bind `lyric:parse` on the binary lane (rpc.bind_bin)"
            );
        }
    }
}
