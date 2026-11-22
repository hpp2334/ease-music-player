//! Plugin entrypoint sources — the single-file vs pkg-tree resolution
//! (phase 4a of the plugin-epoch plan).
//!
//! A manifest's `backend` field and each contribution's `view` field name
//! either a single `.rut` file (the legacy shape — un-migrated plugins
//! keep working unchanged) or a directory carrying `mod.rut` as the
//! pkg-tree root. For a directory the scan reads `mod.rut` and walks its
//! `mod NAME;` declarations to the sibling files — `NAME/mod.rut`
//! (directory form) first, `NAME.rut` (single-file form) second, both
//! present is the loud ambiguity — and the entrypoint registers as a
//! FILE SET (the engine's multi-file module carrier, `RutModuleFiles`
//! tur-side).
//!
//! The walk itself runs on rut-driver's own
//! [`rut_driver::mods::mount_mod_children`] — the exact code the engine's
//! load-time `mount_rows` validation uses — with the filesystem lookups
//! injected here, so scan-side resolution and engine-side validation
//! cannot drift: the engine still refuses a carried-but-undeclared row
//! and a declared-but-missing child loudly at load. A file that fails to
//! PARSE mounts with no children (rut's loader law) — the compile is the
//! loud half, at load.
//!
//! Accepted manifest spellings (all resolve through [`read_entry_source`]):
//!
//! - `"backend": "backend.rut"` — the legacy single file.
//! - `"backend": "backend"` — the directory form (contains
//!   `backend/mod.rut`).
//! - `"backend": "backend/mod.rut"` — naming the module file directly
//!   resolves the tree rooted at its parent directory (same set as the
//!   bare directory spelling).
//!
//! `mod NAME;` children mount under both spellings (`NAME/mod.rut` dir
//! form first, `NAME.rut` single-file form; both present = the loud
//! ambiguity, exactly rut's on-disk loader law).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rut_driver::mods::{mount_mod_children, ChildLookup};
use rut_driver::Mode;

use crate::services::plugin_manager::{unsafe_entry_name, MAX_TOTAL_BYTES};

/// The on-disk module file name of a pkg-tree root (`backend/mod.rut`).
pub const MOD_RUT: &str = "mod.rut";

/// One resolved entrypoint source: what the scan hands to the engine
/// host for registration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntrySource {
    /// A single `.rut` file (the legacy shape) — the file's text.
    Single(String),
    /// A pkg tree: the `mod.rut` root text plus its declared mod children
    /// (mod path → child text, the `RutModuleFiles` grammar). Registers
    /// as a FILE SET on the engine's multi-file rail.
    FileSet {
        root: String,
        mods: BTreeMap<String, String>,
    },
}

/// Resolve + read one entrypoint reference (the manifest's `backend` /
/// `view` value), relative to the plugin dir. The error text is
/// user-actionable and rides the scan's `warnings` verbatim.
pub fn read_entry_source(plugin_dir: &Path, entry: &str) -> Result<EntrySource, String> {
    if unsafe_entry_name(entry) {
        return Err(format!("unsafe entrypoint name '{entry}'"));
    }
    let direct = plugin_dir.join(entry);
    // A directory (or a path naming `…/mod.rut` directly) is the pkg-tree
    // form; the tree is rooted at the directory carrying the module file.
    let tree_root: Option<PathBuf> = if direct.is_dir() {
        Some(direct.clone())
    } else if direct.is_file() && direct.file_name().and_then(|n| n.to_str()) == Some(MOD_RUT) {
        direct.parent().map(Path::to_path_buf)
    } else {
        None
    };
    match tree_root {
        Some(dir) => walk_pkg_tree(&dir),
        None => std::fs::read_to_string(&direct)
            .map(EntrySource::Single)
            .map_err(|e| format!("read {entry}: {e}")),
    }
}

/// Read a directory's `mod.rut` + its declared mod tree (the
/// `mod NAME;` walk, dir-form first / single-file second / both = the
/// loud ambiguity). The whole set is byte-capped at the same budget an
/// installed zip answers for ([`MAX_TOTAL_BYTES`]).
fn walk_pkg_tree(dir: &Path) -> Result<EntrySource, String> {
    let root_path = dir.join(MOD_RUT);
    let root = std::fs::read_to_string(&root_path)
        .map_err(|e| format!("read {}: {e}", root_path.display()))?;
    let dir = dir.to_path_buf();
    // The cycle guard's canonical identities: the physical file paths.
    // Symlinked aliasing resolves to the same identity, so a cycle is
    // the driver's loud `cyclic mod` error, never an endless walk.
    let canon_of = |p: &Path| {
        std::fs::canonicalize(p)
            .unwrap_or_else(|_| p.to_path_buf())
            .display()
            .to_string()
    };
    let mut total = root.len() as u64;
    let mut read = |parent: &str, name: &str| -> Result<ChildLookup, String> {
        let child_path = if parent.is_empty() {
            name.to_string()
        } else {
            format!("{parent}/{name}")
        };
        let dir_form = dir.join(&child_path).join(MOD_RUT);
        let file_form = dir.join(format!("{child_path}.rut"));
        if dir_form.is_file() {
            if file_form.is_file() {
                return Ok(ChildLookup::Ambiguous);
            }
            let text = read_capped(&dir_form, &mut total)?;
            return Ok(ChildLookup::Found {
                text,
                canon: canon_of(&dir_form),
                file: MOD_RUT.to_string(),
            });
        }
        if file_form.is_file() {
            let text = read_capped(&file_form, &mut total)?;
            return Ok(ChildLookup::Found {
                text,
                canon: canon_of(&file_form),
                file: format!("{child_path}.rut"),
            });
        }
        if dir.join(&child_path).is_dir() {
            return Ok(ChildLookup::NoModRut);
        }
        Ok(ChildLookup::Missing)
    };
    let mounted = mount_mod_children(
        &root,
        &canon_of(&root_path),
        Mode::Impl,
        MOD_RUT,
        &mut read,
    )
    .map_err(|e| format!("mount {}: {e}", dir.display()))?;
    Ok(EntrySource::FileSet {
        root,
        mods: mounted
            .into_iter()
            .map(|(path, source)| (path, source.text))
            .collect(),
    })
}

/// Read one module file, tracking the entrypoint's running byte budget
/// (the installed-zip cap — a walk must never read past what an install
/// could have carried).
fn read_capped(path: &Path, total: &mut u64) -> Result<String, String> {
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    *total += text.len() as u64;
    if *total > MAX_TOTAL_BYTES {
        return Err(format!(
            "pkg tree exceeds {MAX_TOTAL_BYTES} bytes — refusing to read past the \
             installed-zip budget ({})",
            path.display()
        ));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal temp-dir guard (parity with `plugin_manager`'s tests — no
    /// extra test deps).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let base = std::env::temp_dir();
            let unique = format!(
                "ease-plugin-pkg-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let path = base.join(unique);
            std::fs::create_dir_all(&path).unwrap();
            TempDir(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The legacy spelling: a single `.rut` file.
    #[test]
    fn single_file_resolves() {
        let guard = TempDir::new();
        std::fs::write(guard.path().join("backend.rut"), "entry fn start() {}").unwrap();
        let src = read_entry_source(guard.path(), "backend.rut").unwrap();
        assert_eq!(src, EntrySource::Single("entry fn start() {}".into()));
    }

    /// The bare-directory spelling (`"backend": "backend"`) and the
    /// module-file spelling (`"backend/mod.rut"`) resolve the SAME tree.
    #[test]
    fn dir_spellings_resolve_the_same_tree() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend")).unwrap();
        std::fs::write(dir.join("backend/mod.rut"), "mod props;\nentry fn start() {}").unwrap();
        std::fs::write(dir.join("backend/props.rut"), "pub fn x() {}").unwrap();

        let a = read_entry_source(dir, "backend").unwrap();
        let b = read_entry_source(dir, "backend/mod.rut").unwrap();
        assert_eq!(a, b, "both spellings mount the same set");
        let EntrySource::FileSet { root, mods } = a else {
            panic!("a directory entrypoint must resolve to a file set");
        };
        assert_eq!(root, "mod props;\nentry fn start() {}");
        assert_eq!(mods.len(), 1);
        assert_eq!(mods["props"], "pub fn x() {}");
    }

    /// The single-file child form (`NAME.rut`) mounts too — and the
    /// directory form wins when both spellings could apply to DIFFERENT
    /// children of the same name (here `props` has only `props.rut`,
    /// `auth` only `auth/mod.rut`).
    #[test]
    fn mixed_child_spellings_mount() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend/auth")).unwrap();
        std::fs::write(dir.join("backend/mod.rut"), "mod props;\nmod auth;\nentry fn start() {}")
            .unwrap();
        std::fs::write(dir.join("backend/props.rut"), "pub fn x() {}").unwrap();
        std::fs::write(dir.join("backend/auth/mod.rut"), "pub fn y() {}").unwrap();

        let EntrySource::FileSet { mods, .. } = read_entry_source(dir, "backend").unwrap() else {
            panic!("file set expected");
        };
        assert_eq!(mods.len(), 2);
        assert_eq!(mods["props"], "pub fn x() {}");
        assert_eq!(mods["auth"], "pub fn y() {}");
    }

    /// rut's loader ambiguity law: `NAME/mod.rut` AND `NAME.rut` both
    /// present is the loud error naming the declaration.
    #[test]
    fn ambiguous_child_is_a_loud_error() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend/props")).unwrap();
        std::fs::write(dir.join("backend/mod.rut"), "mod props;\nentry fn start() {}").unwrap();
        std::fs::write(dir.join("backend/props/mod.rut"), "pub fn a() {}").unwrap();
        std::fs::write(dir.join("backend/props.rut"), "pub fn b() {}").unwrap();

        let err = read_entry_source(dir, "backend").unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");
        assert!(err.contains("props"), "{err}");
    }

    /// A directory present but carrying no module file (and no
    /// single-file sibling) is the loud `NoModRut` error.
    #[test]
    fn dir_without_mod_rut_is_a_loud_error() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend/props")).unwrap();
        std::fs::write(dir.join("backend/props/other.rut"), "pub fn a() {}").unwrap();
        std::fs::write(dir.join("backend/mod.rut"), "mod props;\nentry fn start() {}").unwrap();

        let err = read_entry_source(dir, "backend").unwrap_err();
        assert!(err.contains("mod.rut"), "{err}");
    }

    /// A declared child with neither spelling is the loud `Missing`
    /// error — the engine would refuse the set at load regardless, but
    /// the scan surfaces it first (warnings, zero handles).
    #[test]
    fn missing_child_is_a_loud_error() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::write(dir.join("mod.rut"), "mod ghost;\nentry fn start() {}").unwrap();
        let err = read_entry_source(dir, "mod.rut").unwrap_err();
        assert!(err.contains("ghost"), "{err}");
    }

    /// An entrypoint directory missing its own `mod.rut` is an error.
    #[test]
    fn tree_root_requires_mod_rut() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend")).unwrap();
        std::fs::write(dir.join("backend/props.rut"), "pub fn x() {}").unwrap();
        let err = read_entry_source(dir, "backend").unwrap_err();
        assert!(err.contains("mod.rut"), "{err}");
    }

    /// Nested dir-form trees (`mod a;` in the root, `mod b;` inside
    /// `a/mod.rut`) mount by mod path (`a/b`).
    #[test]
    fn nested_dir_form_mounts_by_path() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend/a/b")).unwrap();
        std::fs::write(dir.join("backend/mod.rut"), "mod a;\nentry fn start() {}").unwrap();
        std::fs::write(dir.join("backend/a/mod.rut"), "mod b;\n").unwrap();
        std::fs::write(dir.join("backend/a/b/mod.rut"), "pub fn deep() {}").unwrap();

        let EntrySource::FileSet { mods, .. } = read_entry_source(dir, "backend").unwrap() else {
            panic!("file set expected");
        };
        assert_eq!(mods.len(), 2);
        assert_eq!(mods["a"], "mod b;\n");
        assert_eq!(mods["a/b"], "pub fn deep() {}");
    }

    /// A mod-cycle on disk is the driver's loud `cyclic mod` error —
    /// never an endless walk. The alias here is a symlink: `mod b;`
    /// resolves to `a/b.rut`, planted as a link back onto `a/mod.rut`
    /// (one canonical identity visited twice).
    #[test]
    fn cyclic_tree_is_a_loud_error() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend/a")).unwrap();
        std::fs::write(dir.join("backend/mod.rut"), "mod a;\n").unwrap();
        std::fs::write(dir.join("backend/a/mod.rut"), "mod b;\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("backend/a/mod.rut"), dir.join("backend/a/b.rut"))
            .unwrap();
        if !cfg!(unix) {
            return; // symlink-less platforms cannot alias; nothing to prove
        }
        let err = read_entry_source(dir, "backend").unwrap_err();
        assert!(err.contains("cyclic"), "{err}");
    }

    /// The entrypoint byte budget: a tree bigger than the installed-zip
    /// cap refuses to read (a scanned entrypoint must never read past
    /// what an install could have carried).
    #[test]
    fn oversized_tree_refuses() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend")).unwrap();
        std::fs::write(dir.join("backend/mod.rut"), "mod big;\nentry fn start() {}").unwrap();
        let big = "pub fn x() {}".repeat((MAX_TOTAL_BYTES / 12 + 16) as usize);
        std::fs::write(dir.join("backend/big.rut"), big).unwrap();
        let err = read_entry_source(dir, "backend").unwrap_err();
        assert!(err.contains("bytes"), "{err}");
    }

    /// The zip-entry name law applies to entrypoint references too:
    /// absolute paths, `..` and backslashes are refused before any read.
    #[test]
    fn unsafe_entry_names_are_refused() {
        let guard = TempDir::new();
        for bad in ["/etc/passwd", "../backend", "a\\b.rut", "a/../b.rut"] {
            let err = read_entry_source(guard.path(), bad).unwrap_err();
            assert!(err.contains("unsafe"), "{bad}: {err}");
        }
    }

    /// A missing entrypoint is a read error naming the reference.
    #[test]
    fn missing_entrypoint_is_a_read_error() {
        let guard = TempDir::new();
        let err = read_entry_source(guard.path(), "backend.rut").unwrap_err();
        assert!(err.contains("read backend.rut"), "{err}");
    }

    /// A broken root parses to no children rut-side (the loader mounts
    /// nothing for a file that doesn't parse — the compile reports the
    /// syntax loudly at LOAD); the resolution itself succeeds.
    #[test]
    fn broken_root_mounts_no_children() {
        let guard = TempDir::new();
        let dir = guard.path();
        std::fs::create_dir_all(dir.join("backend")).unwrap();
        std::fs::write(dir.join("backend/mod.rut"), "mod props; totally not rut !!!").unwrap();
        std::fs::write(dir.join("backend/props.rut"), "pub fn x() {}").unwrap();
        let src = read_entry_source(dir, "backend").unwrap();
        let EntrySource::FileSet { mods, .. } = src else {
            panic!("file set expected");
        };
        assert!(mods.is_empty(), "a non-parsing root mounts no children");
    }
}
