//! Compile + boot validation for the shipped rut plugin sources.
//!
//! Every plugin under `plugins/<id>/` must compile and boot on the host
//! harness: `backend.rut` loads with the FULL app plugin set
//! (`EaseMusicPlugin` included — its rows register, though Android-only
//! upcalls are unreachable and must not fire at boot) and every
//! contribution `view.rut` loads into a fresh instance (views never call
//! rows at boot — they build UI in `start`).
//!
//! This is the plugin toolchain: there is no rspack/pnpm build anymore —
//! the engine compiles the sources, and this suite is what `pnpm test`
//! runs to catch a broken plugin before it ever reaches a device.

// This crate is a pure test harness — all logic lives behind cfg(test) so
// the (empty) lib target builds without the dev-dependencies.
#[cfg(test)]
mod plugin_checks {
    use std::path::{Path, PathBuf};

    use ease_client_android::plugin_runtime::EaseMusicPlugin;
    use ease_client_backend::ctx::BackendContext;
    use ease_tur_rpc::TurRpcBusPlugin;
    use tur_engine::core::plugin::Plugin;
    use tur_integration_tests::TurTestApp;

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

    /// The app plugin set, host-buildable: the bus rows + the full `ease`
    /// surface (bound to an uncontexted backend — rows that need DB/JNI
    /// never run at boot).
    fn app_plugins() -> Vec<Box<dyn Plugin>> {
        vec![
            Box::new(TurRpcBusPlugin::new(None)),
            Box::new(EaseMusicPlugin::new(BackendContext::new())),
        ]
    }

    /// Load `source` into a fresh harness instance; panic with the compile
    /// diagnostics on failure (rut compile errors name the row + line).
    fn assert_loads(source: &str, what: &str) {
        let mut app = TurTestApp::new_with_extra_plugins(400.0, 400.0, app_plugins())
            .expect("harness app");
        if let Err(e) = app.load_rut_module(source) {
            panic!("{what} failed to compile/boot: {e}");
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
            assert_eq!(api, 2, "{id}: manifest apiVersion must be 2 (the rut epoch)");

            if let Some(backend) = manifest["backend"].as_str() {
                let src = std::fs::read_to_string(dir.join(backend))
                    .unwrap_or_else(|e| panic!("{id}: backend source {backend} unreadable: {e}"));
                assert_loads(&src, &format!("{id}/{backend}"));
                checked += 1;
            }
            let views: Vec<&str> = ["storages", "dashboard"]
                .iter()
                .filter_map(|kind| manifest["contributions"][kind].as_array())
                .flat_map(|arr| arr.iter())
                .filter_map(|c| c["view"].as_str())
                .collect();
            for view in views {
                let src = std::fs::read_to_string(dir.join(view))
                    .unwrap_or_else(|e| panic!("{id}: view source {view} unreadable: {e}"));
                assert_loads(&src, &format!("{id}/{view}"));
                checked += 1;
            }
        }
        assert!(!plugin_dirs().is_empty(), "no plugins found");
        assert!(checked > 0, "no rut sources checked — the plugins are gone?");
    }

    /// The contract ops every storage backend must serve (the literals
    /// `ease-js-storage` dispatches). A backend with `storages`
    /// contributions must register all four.
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
            let src = std::fs::read_to_string(dir.join(backend)).expect("backend readable");
            for op in [
                "storage:list",
                "storage:get",
                "storage:get-chunk",
                "storage:release",
            ] {
                assert!(
                    src.contains(&format!("\"{op}\"")),
                    "{id}: storage backend must register the contract op `{op}`"
                );
            }
        }
    }

    /// Lyric-parser backends must register `lyric:parse` (the literal the
    /// Rust lyric dispatch calls).
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
            let src = std::fs::read_to_string(dir.join(backend)).expect("backend readable");
            assert!(
                src.contains("\"lyric:parse\""),
                "{id}: lyric parser backend must register `lyric:parse`"
            );
        }
    }
}
