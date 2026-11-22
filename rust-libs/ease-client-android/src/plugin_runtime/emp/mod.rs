//! `emp` — the Ease Music Plugin SDK surface (phase 2 of the
//! plugin-epoch plan): the `emp_host` host pkg (Rust-owned service
//! rows) + the `emp` class pkg (the rut SDK plugins import).
//!
//! # The identity law — constructor-injected handle
//!
//! Every scoped `emp_host` row carries a LEADING `opaque` parameter —
//! the **plugin handle** — and every `emp` service class holds it as
//! its one field (`pub class Db { h: opaque }`). The handle is
//! host-minted and delivered as `start`'s argument (the phase-1
//! engine lane): the embedder stamps a [`PluginHandle`] into the
//! instance's data at build time and boots the module with the
//! handle's u64 id (`start(cx)`), so rut code never mints one and can
//! never borrow another plugin's — opaques are per-VM, and the row
//! bodies decode the box against the CALLING INSTANCE's own stamp,
//! trapping `emp:{ns}:` on anything else:
//!
//! ```rut
//! // plugin backend — the whole identity dance
//! entry fn start(cx: opaque) {              // no return: the answer rail is NOT the identity rail
//!     const db = Db(cx);                    // services constructed FROM the handle
//!     // …
//! }
//! ```
//!
//! The u64 IS the whole crossing (the phase-1 lane's law: an
//! `OpaqueRef` is heap-backed and cannot travel the embedder → worker
//! channel), so the decode lane is "the engine-sealed u64 box whose
//! value equals the instance's own handle id". A rut-side
//! `opaque(<u64>)` forgery can only ever name the forger's OWN id
//! (ids are per-instance stamps, never exposed across instances) —
//! which resolves to exactly the identity it already has; a foreign
//! id or any other box shape traps.
//!
//! # One owner per host pkg
//!
//! [`install`] pushes `emp_host`'s decl rows + offered mounts in BOTH
//! phases and its body pkg only at boot — `install_host_pkg` sees the
//! spec exactly once. The rpc bind-table rows (`rpc::bind` / `rpc::handler`
//! / `rpc::call`) are part of that one mount: their bodies relocate from
//! `ease_tur_rpc::install_rows` (phase 3), and `emp` ships the `Rpc`
//! class over them.
//!
//! # Boot law
//!
//! `Db(cx)`-style construction is safe at boot: constructors only
//! store the handle — no Android-only rows, no network at start.

pub mod rows;

use rut_driver::Mounted;
use tur_engine::core::rut_runtime::{mount_host_tree, RutHandles, RutPkgCx};
use rut_vm::TrapKind;

use crate::plugin_runtime::{PluginId, PluginInstance};

/// The `emp` pkg sources (`src/mod.rut` + its manifest — one source of
/// truth; the manifest rides the packer's dep heads).
const EMP_RUT: &str = include_str!("src/mod.rut");
const EMP_MANIFEST: &str = include_str!("src/rut.jsonc");

// ---------------------------------------------------------------------------
// PluginHandle — the host-minted identity
// ---------------------------------------------------------------------------

/// Handle sequence (per process). Small sequential ids keep the
/// engine's entry-slot mint space sane (minted answer tokens climb
/// above the boot identity — a near-`u64::MAX` identity would
/// saturate the mint counter); unforgeability comes from the
/// per-VM stamp check, not the id's entropy (see the decode below).
static HANDLE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// The host-minted plugin identity: the payload behind every `emp_host`
/// row's leading `opaque` parameter. Minted at instance build time
/// (headless spawn / view `createInstance`) from the instance's
/// [`PluginId`] / [`PluginInstance`] stamps, stamped into the
/// instance's data slot, and delivered to `start(cx)` as the sealed u64
/// id ([`PluginHandle::id`]). Script-unforgeable across plugins: the
/// row bodies decode the box against the calling instance's own stamp.
#[derive(Debug, Clone)]
pub struct PluginHandle {
    /// The per-instance identity — the u64 the engine seals into the
    /// boot box (`unopaque<u64>(cx)` recovers it module-side).
    id: u64,
    /// The calling plugin (from the instance-data stamp at mint time).
    plugin_id: PluginId,
    /// The instance's storage identity (`None` = create-mode view or
    /// the headless backend).
    instance: PluginInstance,
}

impl PluginHandle {
    /// Mint a fresh handle (the next sequence id) for the given plugin
    /// identity. The caller stamps the handle into the instance's data
    /// and boots the module with [`PluginHandle::id`].
    pub fn mint(plugin_id: PluginId, instance: PluginInstance) -> Self {
        Self {
            id: HANDLE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            plugin_id,
            instance,
        }
    }

    /// The sealed boot identity (the u64 the embedder passes as
    /// `start_cx`).
    pub fn id(&self) -> u64 {
        self.id
    }

    /// The calling plugin's stable id.
    pub fn plugin_id(&self) -> &str {
        self.plugin_id.as_str()
    }

    /// The instance's storage identity (`None` = create-mode view or
    /// the headless backend) — [`super::PluginInstance`]'s payload.
    pub fn storage_instance(&self) -> Option<&str> {
        self.instance.0.as_deref()
    }
}

/// Decode a row's leading handle opaque against the calling instance's
/// own stamp. `ns` names the row family for the trap message.
///
/// Two legal shapes, both verified against the instance's own
/// [`PluginHandle`]:
/// 1. the engine-sealed boot box (a rut-value box over `TY_U64`) whose
///    value is the instance's own handle id — the production lane
///    (`Db(cx)` at boot),
/// 2. a genuine host-payload box over [`PluginHandle`] with the same id
///    (nothing mints one yet — the phase-3 lane's door, and the unit
///    tests' direct mint).
///
/// Anything else — a foreign id, a host box of another payload type, a
/// rut box over a non-u64 — traps `emp:{ns}:`.
pub(crate) fn decode_handle(
    vm: &rut_vm::interp::Vm,
    h: &rut_vm::OpaqueRef,
    ns: &str,
    handles: &std::rc::Rc<RutHandles>,
) -> Result<PluginHandle, rut_vm::Trap> {
    let foreign = |why: String| {
        rut_vm::Trap::new(
            TrapKind::Invalid,
            format!("emp:{ns}: foreign handle opaque — services take the start(cx) handle ({why})"),
        )
    };
    let own = handles.inst.data::<PluginHandle>().ok_or_else(|| {
        rut_vm::Trap::new(
            TrapKind::Invalid,
            format!(
                "emp:{ns}: no plugin handle bound to this instance (spawned without the emp stamp)"
            ),
        )
    })?;

    // 1. the engine-sealed boot identity (a rut-value box over a u64).
    if let Some((slot, ty)) = vm.opaque_rut_value(rut_vm::Slot { r: h.ptr() }) {
        if ty == rut_core::types::TY_U64 {
            // The seal wrote the i64 bit pattern (`Slot::int`); the id is
            // the same bits read as u64.
            let id = unsafe { slot.i } as u64;
            if id == own.id {
                return Ok(own.clone());
            }
            return Err(foreign(format!("id {id} is not this instance's handle")));
        }
        return Err(foreign(format!("sealed value is {ty:?}, not u64")));
    }

    // 2. the host-payload box (the direct-mint door).
    if let Ok(boxed) = rut_vm::Opaque::<PluginHandle>::from_handle(h) {
        let got = boxed.with(|p| p.clone())?;
        if got.id == own.id {
            return Ok(got);
        }
        return Err(foreign(format!("host box names handle id {}", got.id)));
    }

    Err(foreign("the box holds neither the boot identity nor a PluginHandle".to_string()))
}

// ---------------------------------------------------------------------------
// the installer
// ---------------------------------------------------------------------------

/// The `emp` pkg's offered mount (source body + its parsed manifest —
/// the dep heads are the manifest's truth).
fn emp_mount() -> Mounted {
    let manifest = rut_driver::parse_manifest(EMP_MANIFEST)
        .expect("the emp manifest always parses");
    let spec = manifest.name.clone().expect("the emp manifest names itself");
    Mounted::source(&spec, EMP_RUT).manifest(manifest)
}

/// The `emp_host` host pkg's offered mount: the decl rows +
/// [`mount_host_tree`] (the module tree — `use emp_host::db::single_get`
/// spells the tree). The Rust rows are the compile truth.
fn emp_host_mount() -> Mounted {
    let mut m = Mounted::host("emp_host", rows::decl());
    mount_host_tree(&mut m);
    m
}

/// The installer (a `RutPkgExt`, pushed by [`super::plugin`]). Called
/// twice by the engine (decl probe, then boot): the offered mounts push
/// in both (the compile needs the surfaces), the body pkg only at boot.
pub fn install(cx: &mut RutPkgCx<'_>) {
    cx.offered.push(emp_host_mount());
    cx.offered.push(emp_mount());

    let Some(handles) = cx.handles else {
        return; // compile-time decl probe — bodies install at boot only
    };
    // One owner: `emp_host` installs exactly once, here.
    cx.boot_pkgs.push(rows::pkg(handles));
}
