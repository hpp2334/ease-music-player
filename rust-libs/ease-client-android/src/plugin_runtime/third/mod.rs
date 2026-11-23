//! The third-party utility libs — the identity-free utilities that
//! left the app SDK (phase 2 of the plugin-epoch plan). Each lib is a
//! **standard rut pkg folder** under `plugins/3rd/<name>/` (rut.jsonc +
//! src/mod.rut — the pinned pkg grammar's `src/` shell), wrapped over
//! a small host pkg (`plugins/3rd/<name>_host/`, a pure declaration
//! surface lowered from its `src/mod.d.rut`).
//!
//! ONE SOURCE OF TRUTH: this module `include_str!`s the `plugins/3rd`
//! sources (the lib bodies, the manifests, the host decl surfaces) —
//! the exact bytes the phase-6 packer embeds into every plugin's
//! `dist/*.zip` closure — and offers them into the compile session, so
//! `use xml::to_json`... resolves by name on device exactly as
//! `use emp::Db` does. A pkg-zip load mounts the archive's embedded
//! copies too, but the offered set mounts FIRST (first-mount-wins), so
//! the session's mounts answer — embedded duplicates ride inert (and
//! byte-identical). The row DECLS are the `.d.rut` files; the BODIES
//! live here (`crypto` / `xml` / `net` / `time`), and the boot's
//! `verify_against` traps any drift between the two.
//!
//! Installer shape ([`install`], a `RutPkgExt` pushed by `EaseMusicPlugin`
//! beside `emp::install`): the offered mounts push in BOTH phases (the
//! compile probe needs the surfaces), the body pkgs only at boot —
//! one owner per host pkg spec (`crypto_host` et al install exactly
//! once).
//!
//! Identity law: NOTHING here resolves a plugin — no leading handle
//! opaque, no instance-data slot. These libs are app-owned utilities.

use std::collections::BTreeMap;

use rut_driver::Mounted;
use tur_engine::core::rut_runtime::RutPkgCx;

pub mod crypto;
pub mod net;
pub mod time;
pub mod xml;

/// The lib pkg sources (`plugins/3rd/<name>/src/mod.rut`).
const CRYPTO_RUT: &str = include_str!("../../../../../plugins/3rd/crypto/src/mod.rut");
const XML_RUT: &str = include_str!("../../../../../plugins/3rd/xml/src/mod.rut");
const NET_RUT: &str = include_str!("../../../../../plugins/3rd/net/src/mod.rut");
const TIME_RUT: &str = include_str!("../../../../../plugins/3rd/time/src/mod.rut");

/// The lib manifests (the deps tables ride them — the packer's truth).
const CRYPTO_MANIFEST: &str = include_str!("../../../../../plugins/3rd/crypto/rut.jsonc");
const XML_MANIFEST: &str = include_str!("../../../../../plugins/3rd/xml/rut.jsonc");
const NET_MANIFEST: &str = include_str!("../../../../../plugins/3rd/net/rut.jsonc");
const TIME_MANIFEST: &str = include_str!("../../../../../plugins/3rd/time/rut.jsonc");

/// The host decl surfaces (`plugins/3rd/<name>_host/src/mod.d.rut`) —
/// the rows the bodies bind, lowered like the vendored `async_host`.
const CRYPTO_HOST_D_RUT: &str = include_str!("../../../../../plugins/3rd/crypto_host/src/mod.d.rut");
const XML_HOST_D_RUT: &str = include_str!("../../../../../plugins/3rd/xml_host/src/mod.d.rut");
const NET_HOST_D_RUT: &str = include_str!("../../../../../plugins/3rd/net_host/src/mod.d.rut");
const TIME_HOST_D_RUT: &str = include_str!("../../../../../plugins/3rd/time_host/src/mod.d.rut");

/// The installer (a `RutPkgExt`). Called twice by the engine (decl
/// probe, then boot).
pub fn install(cx: &mut RutPkgCx<'_>) {
    // The offered set: each lib pkg + its host decl pkg. The lib
    // manifests carry their dep heads (`crypto` → `crypto_host`).
    let source = |manifest_text: &str, text: &str| -> Mounted {
        let manifest = rut_driver::parse_manifest(manifest_text)
            .unwrap_or_else(|e| panic!("a 3rd lib manifest must parse: {e}"));
        let spec = manifest
            .name
            .clone()
            .expect("a 3rd lib manifest names itself");
        Mounted::source(&spec, text).manifest(manifest)
    };
    cx.offered.push(source(CRYPTO_MANIFEST, CRYPTO_RUT));
    cx.offered.push(source(XML_MANIFEST, XML_RUT));
    cx.offered.push(source(NET_MANIFEST, NET_RUT));
    cx.offered.push(source(TIME_MANIFEST, TIME_RUT));
    for (spec, text) in [
        ("crypto_host", CRYPTO_HOST_D_RUT),
        ("xml_host", XML_HOST_D_RUT),
        ("net_host", NET_HOST_D_RUT),
        ("time_host", TIME_HOST_D_RUT),
    ] {
        cx.offered.push(host_mount(spec, text));
    }

    // The bodies: one owner per host pkg spec — installed exactly once
    // (boot only; the compile probe ignores `boot_pkgs`).
    let Some(handles) = cx.handles else {
        return;
    };
    cx.boot_pkgs.push(crypto::pkg());
    cx.boot_pkgs.push(xml::pkg());
    cx.boot_pkgs.push(net::pkg(handles));
    cx.boot_pkgs.push(time::pkg());
}

/// Lower a vendored host decl surface into its offered `Mounted` (the
/// `async_host` precedent — the `.d.rut` is the rows' compile truth;
/// the bodies bind scope-relative and the boot verifies the join).
fn host_mount(spec: &str, text: &str) -> Mounted {
    let mut m = rut_driver::lower_decl_module(text, &BTreeMap::new(), "mod.d.rut")
        .unwrap_or_else(|e| panic!("the vendored {spec} decl always lowers: {e}"));
    m.spec = spec.to_string();
    m
}
