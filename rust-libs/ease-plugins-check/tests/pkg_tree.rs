//! Phase-4a fixture: a pkg-tree plugin entrypoint — a DIRECTORY whose
//! `mod.rut` roots the tree, `mod NAME;` children beside it — resolves
//! through the scan's own lane (`read_entry_source`) and boots as a file
//! set through the FULL app plugin set: the exact path a device's scan →
//! `register_files` → multi-file load runs.
//!
//! The fixture lives under this crate's test dirs (NOT under `plugins/`
//! — the shipped set stays unmigrated until phase 4b).

use std::path::{Path, PathBuf};

use ease_client_android::plugin_runtime::EaseMusicPlugin;
use ease_client_backend::ctx::BackendContext;
use ease_client_backend::services::plugin_pkg::{read_entry_source, EntrySource};
use ease_tur_rpc::TurRpcBindPlugin;
use tur_engine::core::plugin::Plugin;
use tur_engine::core::rut_runtime::RutModuleFiles;
use tur_integration_tests::TurTestApp;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pkg-plugin")
        .canonicalize()
        .expect("fixture dir")
}

/// The app plugin set, host-buildable (parity with the lib harness).
fn app_plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(TurRpcBindPlugin::new()),
        Box::new(EaseMusicPlugin::new(BackendContext::new())),
    ]
}

/// Boot one resolved entrypoint; panic with the compile diagnostics on
/// failure. Single file = the single rail, file set = the engine's
/// multi-file rail.
fn assert_boots(source: EntrySource, what: &str) {
    let app =
        TurTestApp::new_with_extra_plugins(400.0, 400.0, app_plugins()).expect("harness app");
    let result = match source {
        EntrySource::Single(src) => app.load_rut_module(&src),
        EntrySource::FileSet { root, mods } => app.load_rut_module_files(RutModuleFiles {
            root: std::sync::Arc::from(root.as_str()),
            mods: mods
                .into_iter()
                .map(|(path, text)| {
                    (
                        std::sync::Arc::from(path.as_str()),
                        std::sync::Arc::from(text.as_str()),
                    )
                })
                .collect(),
        }),
    };
    if let Err(e) = result {
        panic!("{what} failed to compile/boot: {e}");
    }
}

/// The two-file fixture boots end to end: the manifest's `backend` names
/// the DIRECTORY (`"backend"`), the scan lane resolves it to a file set
/// (root + the `props` child), and the set compiles + boots with the
/// app plugin set — the root consumes the child's fn at boot, so a lost
/// mount fails the compile loudly instead of silently booting root-only.
#[test]
fn pkg_tree_entrypoint_boots_through_the_app_plugin_set() {
    let dir = fixture_root();

    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("manifest.json")).expect("manifest readable"),
    )
    .expect("manifest parses");
    assert_eq!(
        manifest["apiVersion"].as_i64(),
        Some(2),
        "the fixture declares the rut epoch (phase 5 bumps the assert)"
    );

    let backend = manifest["backend"].as_str().expect("backend field");
    assert_eq!(backend, "backend", "the fixture declares the DIRECTORY spelling");

    let source = read_entry_source(&dir, backend).expect("entrypoint resolves");
    let EntrySource::FileSet { root, mods } = &source else {
        panic!("a directory entrypoint must resolve to a file set, got {source:?}");
    };
    assert!(root.contains("mod props;"), "the root is mod.rut: {root}");
    assert!(
        mods.get("props").is_some_and(|t| t.contains("pub fn greeting")),
        "the props child mounted: {:?}",
        mods.keys().collect::<Vec<_>>()
    );
    // The module-file spelling (`backend/mod.rut`) mounts the same set —
    // both manifest spellings 4b migrations may use are equivalent.
    let twin = read_entry_source(&dir, "backend/mod.rut").expect("module-file spelling");
    assert_eq!(source, twin, "both spellings resolve the same tree");
    assert_boots(source, "pkg-plugin/backend");
}
