//! The plugin packer — the build half of the phase-6 dist pipeline.
//!
//! Walks every plugin under `plugins/` (manifest.json + apiVersion 3),
//! packs each entrypoint pkg with the **source-mode zip** lane — the
//! exact APIs `rut pack --mode source` shells to
//! ([`rut_native::pack_dir_zip`]: `rut.jsonc` + `src/**` + the path-dep
//! closure embedded under `deps/`, every row rewritten to point inside,
//! the walk validating the whole closure first) — and writes
//! `dist/backend.zip` / `dist/view.zip` beside the manifest.
//!
//! Dep grammar (the phase-6 decision): an entrypoint's `rut.jsonc`
//! declares only its **path deps** (`../../3rd/<lib>` — embedded). The
//! **session deps** (`emp`, `json`, the host pkgs) are deliberately NOT
//! declared: they resolve against the engine's offered set at device
//! compile through the pkg-zip lane's ambient-nameability law (the file
//! lane's exact shape), and rut's grammar has no descriptor for
//! "satisfied by the session" — `rut pack` embeds every row it walks,
//! and the pack-time closure must resolve offline. Staging `emp` et al.
//! would embed the SDK into every plugin zip (and the host pkgs have no
//! `.d.rut` to stage at all), so the clean grammar shape won: declare
//! what embeds, let the session supply the rest.
//!
//! `scripts/package-plugin.ts` then zips `manifest.json + dist/*.zip +
//! icons` into the registry zip; `dist/` stays gitignored — the registry
//! zips + `plugins.json` are the committed artifacts.

use std::path::{Path, PathBuf};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let root = match args.get(1) {
        Some(p) => PathBuf::from(p),
        None => Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins")
            .canonicalize()
            .expect("plugins dir"),
    };
    if let Err(errs) = run(&root) {
        for e in &errs {
            eprintln!("error: {e}");
        }
        std::process::exit(1);
    }
}

fn run(root: &Path) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    let mut packed = 0usize;
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .unwrap_or_else(|e| panic!("plugins dir {}: {e}", root.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|dir| dir.is_dir() && dir.join("manifest.json").is_file())
        .collect();
    dirs.sort();
    if dirs.is_empty() {
        return Err(vec![format!("no plugins found under {}", root.display())]);
    }
    for dir in dirs {
        match pack_plugin(&dir) {
            Ok(count) => packed += count,
            Err(e) => errors.push(e),
        }
    }
    if errors.is_empty() {
        println!("packed {packed} entrypoint artifact(s)");
        Ok(())
    } else {
        Err(errors)
    }
}

/// Pack one plugin's entrypoints. Returns the artifact count.
fn pack_plugin(dir: &Path) -> Result<usize, String> {
    let manifest_path = dir.join("manifest.json");
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("{}: {e}", manifest_path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
    let id = manifest["id"].as_str().ok_or("manifest.json has no id")?;
    let api = manifest["apiVersion"].as_i64().unwrap_or(0);
    if api != 3 {
        return Err(format!("{id}: manifest apiVersion must be 3 (the emp epoch)"));
    }
    let mut entries: Vec<String> = Vec::new();
    if let Some(b) = manifest["backend"].as_str() {
        if !b.is_empty() {
            entries.push(b.to_string());
        }
    }
    for kind in ["storages", "dashboard"] {
        if let Some(list) = manifest["contributions"][kind].as_array() {
            for c in list {
                if let Some(v) = c["view"].as_str() {
                    if !v.is_empty() {
                        entries.push(v.to_string());
                    }
                }
            }
        }
    }
    if entries.is_empty() {
        return Err(format!("{id}: manifest names no entrypoints"));
    }
    // dist/ is pure build output — fresh each run (the stale phase-5
    // staging layout never lingers beside the packed artifacts).
    let out_dir = dir.join("dist");
    if out_dir.exists() {
        std::fs::remove_dir_all(&out_dir)
            .map_err(|e| format!("{}: {e}", out_dir.display()))?;
    }
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("{}: {e}", out_dir.display()))?;
    let mut packed = 0usize;
    for entry in &entries {
        // The manifest PREFERs dist pointers ("backend.zip") — the pack
        // source is the pkg tree of the same stem. A source spelling
        // ("backend") packs equally (the scan accepts both).
        let stem = entry.strip_suffix(".zip").unwrap_or(entry);
        let pkg_dir = dir.join(stem);
        if !pkg_dir.join("rut.jsonc").is_file() {
            return Err(format!(
                "{id}/{stem}: no rut.jsonc — the entrypoint must be a rut pkg \
                 (rut.jsonc + src/ shell)"
            ));
        }
        let bytes = rut_native::pack_dir_zip(&pkg_dir)
            .map_err(|e| format!("{id}/{stem}: rut pack failed: {e}"))?;
        let out = out_dir.join(format!("{stem}.zip"));
        std::fs::write(&out, &bytes).map_err(|e| format!("{}: {e}", out.display()))?;
        println!(
            "packed {id}/{stem} -> {} ({} bytes)",
            out.display(),
            bytes.len()
        );
        packed += 1;
    }
    Ok(packed)
}
