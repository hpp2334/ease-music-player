//! `ease-tur-rpc` — the host⇄rut plugin RPC bridge.
//!
//! The tur engine's JS-era event bus is gone; this crate re-implements the
//! request/response + event transport over the **rut entry rail**, keeping
//! the `RpcClient` surface (`call_host` / `call_host_bytes` / `emit_event`)
//! the backend services already use. There is no injected script module and
//! no JS anywhere: the contract with a plugin backend is a **rut entry-fn
//! convention**, served in one of two shapes — the phase-3 redesign is
//! capability-driven and both stay live until the old one is retired:
//!
//! # The bind-table contract (new — `emp`'s `Rpc` class)
//!
//! A backend serves ALL ops through **one** dispatch entry per lane,
//! declared once in the app module:
//!
//! ```rut
//! use emp::Rpc;
//!
//! entry fn start(cx: opaque) {
//!     const db = Db(cx);
//!     const rpc = Rpc(cx);
//!     rpc.bind("storage:list", fn (args: str) -> ?str { return storage_list(db, args); });
//!     rpc.bind_bin("lyric:parse", fn (args: bytes) -> ?bytes { return lyric_parse(db, args); });
//! }
//!
//! entry fn rpc(cx: opaque, op: str, args: str) -> ?str {
//!     const rpc = Rpc(cx);
//!     return rpc.dispatch(op, args);
//! }
//!
//! entry fn rpc_bin(cx: opaque, op: str, args: bytes) -> ?bytes {
//!     const rpc = Rpc(cx);
//!     return rpc.dispatch_bin(op, args);
//! }
//! ```
//!
//! `args` is RAW — the handler decodes it (`json` lib for str ops, plain
//! bytes for the binary lane); the entry returns the raw payload
//! (`nil` = "not mine", the lyric dispatch's fall-through) and a handler
//! error propagates as a rut trap whose message keeps the `UNAUTHORIZED` /
//! `TIMEOUT` prefix law. **The envelope (`{"ok":…}`) is built HERE, on the
//! host, from the value/trap — no envelope code exists in rut.** The sealed
//! handler closures live in a host-side per-instance table
//! ([`RpcHandlers`], filed through `rpc.bind`'s row); the `cx` entry lane
//! delivers the embedder-supplied handle, so dispatch needs no module
//! state to find the identity.
//!
//! # The stash-bus contract (old — un-migrated plugins)
//!
//! Each plugin backend instance carries a per-instance [`RpcBus`] (stamped
//! as instance data by the embedder at `createInstance`):
//!
//! - `ops: op → rut export name` — the backend registers each contract op
//!   as its own `entry fn` (`ease.rpc.register("storage:list", "op_…")`).
//! - `events: type → [export names]` — `ease.rpc.subscribe_event`.
//! - `inbox: single pending request` — the host stashes
//!   `{"op":…,"args":…}` before firing the entry; the rut side pulls it
//!   with `ease.rpc.take_request()` and answers the reply envelope.
//!
//! # Dual-mode dispatch
//!
//! [`RpcClient`] picks the lane per instance with a **lazy capability
//! probe** (the detection point is recorded on the client): on first use
//! it fires an inert probe op at the `rpc` / `rpc_bin` entries — a reply
//! (typically the nil "not bound" arm) proves the export, the engine's
//! `no export` refusal proves its absence. Entries present ⇒ the
//! bind-table lane; both absent ⇒ the stash bus. Phase 5 deletes the bus
//! lane; the split lives entirely inside [`RpcClient::call_host`] /
//! [`RpcClient::call_host_bytes`] / [`RpcClient::emit_event`].
//!
//! # Streaming (chunked pull)
//!
//! There are no credit-gated push streams. The storage contract
//! (`ease-js-storage`) splits the old `storage:get` opener into three
//! plain ops driven by the host: `storage:get` (opener → `{sid, meta}`),
//! `storage:get-chunk` (`{sid, offset, len}` → `{data: <base64>, eof}`),
//! `storage:release` (`{sid}`). Pull = natural backpressure; str ops keep
//! the base64 crossing (the binary lane exists for the ops that want raw
//! bytes, e.g. `lyric:parse` after migration).
//!
//! # Threading
//!
//! [`RpcClient`] is `Clone` + `Send`; every call serializes through a
//! shared gate (the old lane's inbox is single-slot; the entry rail is one
//! VM call at a time) and crosses onto the instance's tur-host thread via
//! the [`RutDriver`] impl (a blocking round-trip, the same pattern tur's
//! own `loadModule` uses). Never call back into the *same* instance from
//! inside a dispatch (single-slot inbox + face re-entrancy). The sealed
//! closures in [`RpcHandlers`] are `!Send` on purpose — they are VM-heap
//! cells and never leave the instance's thread.
//!
//! # Testability
//!
//! [`RutDriver`] abstracts the engine hop, so host tests drive a real rut
//! module through the plugins + `TurTestApp` without Android.
//! [`TurRpcBindPlugin`] mounts the bind-table rows standalone (the same
//! bodies `emp_host` installs) for fixtures that exercise the new lane
//! without the full SDK surface.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;

// ---------------------------------------------------------------------------
// The bind-table contract's constants (the rut side spells the same
// literals — the `emp` class composes the bin/event key prefixes, the host
// composes the event key; keep the four in sync).
// ---------------------------------------------------------------------------

/// The str dispatch entry's name — `entry fn rpc(cx, op, args) -> ?str`.
pub const DISPATCH_ENTRY: &str = "rpc";
/// The binary dispatch entry's name — `entry fn rpc_bin(cx, op, args) -> ?bytes`.
pub const DISPATCH_ENTRY_BIN: &str = "rpc_bin";
/// Key prefix for binary-lane handlers (the `emp` class composes
/// `bin:<op>` so both lanes can share one table).
pub const BIN_KEY_PREFIX: &str = "bin:";
/// Key prefix for event handlers (`event:<type>`) — composed host-side by
/// [`RpcClient::emit_event`], rut-side by `Rpc.subscribe`.
pub const EVENT_KEY_PREFIX: &str = "event:";
/// The capability probe's op — inert by construction: the bind table has
/// no handler bound under it (contract ops are `domain:verb` literals or
/// `bin:` / `event:` prefixed keys), so a probe dispatch answers the nil
/// "not bound" arm without running any handler. Reserved namespace.
const PROBE_OP: &str = "__tur_rpc_probe";
/// The engine's missing-export refusal (`RutRuntime::export_ret`'s error,
/// verbatim) — the capability probe's discriminator.
const NO_EXPORT_MARKER: &str = "no export";
/// The engine's trap decode appends `{entry}: {kind}` before the trap
/// message after this separator — [`trap_message`] splits it back off.
const TRAP_SEPARATOR: &str = " \u{2014} "; // " — "

/// Split the plugin's trap message out of the engine's eval decode
/// (`module eval: rpc: Panic — UNAUTHORIZED: expired`). Everything after
/// the FIRST separator is the trap's own message — the envelope error
/// arm's payload, whose head the `UNAUTHORIZED` / `TIMEOUT` prefix law
/// reads. Non-trap failures (worker gone, stale instance) carry no
/// separator and pass through verbatim as transport errors.
fn trap_message(err: &str) -> &str {
    match err.split_once(TRAP_SEPARATOR) {
        Some((_, msg)) => msg,
        None => err,
    }
}

/// Map a new-lane dispatch failure onto the client error: a rut trap is
/// the plugin's own failure ([`RpcError::Handler`] — the envelope error
/// arm), anything else is transport.
fn rpc_error_of_transport(err: String) -> RpcError {
    if err.contains(TRAP_SEPARATOR) {
        RpcError::Handler(trap_message(&err).trim().to_string())
    } else {
        RpcError::Transport(err)
    }
}

// ---------------------------------------------------------------------------
// The bind-table rows: identity, the sealed-handler table, decls + bodies.
// ---------------------------------------------------------------------------

/// The rpc rows' identity stamp: the u64 the embedder minted for the
/// instance — the SAME value [`RpcClient`] dispatches with (`start_cx`).
///
/// The narrow view of the SDK's `PluginHandle` id: the bind-table rows
/// live in THIS crate (one owner — `emp_host` mounts them through
/// [`install_rows`]), so the identity law they enforce is defined here.
/// The embedder stamps both from the same minted id; a rut-side
/// `opaque(<u64>)` forgery can only name the forger's own id, a foreign
/// id or any other box shape traps `emp:rpc:`.
#[derive(Clone)]
pub struct RpcIdentity(pub u64);

/// The sealed-handler table — one per instance, VM-confined.
///
/// `rpc.bind(key, f)` seals the rut closure into an opaque box (the
/// `rs_watch_mut` precedent — closures cross as sealed opaques) and the
/// row files it here keyed by the bind key (the op, `bin:<op>`, or
/// `event:<type>`); the dispatch entries fetch it back and invoke it. The
/// table rides the per-instance plugin state (the handle names the
/// instance only for the identity law), so `emp` needs no module-global
/// state. `OpaqueRef` owns an arena reference — holding one keeps the
/// sealed closure alive for the instance's lifetime.
#[derive(Default)]
pub struct RpcHandlers {
    map: HashMap<String, rut_vm::OpaqueRef>,
}

impl RpcHandlers {
    /// File a sealed handler under `key` (re-binding replaces).
    pub fn bind(&mut self, key: &str, f: rut_vm::OpaqueRef) {
        self.map.insert(key.to_string(), f);
    }

    /// The sealed handler bound under `key` (`None` = not bound — the
    /// dispatch entry answers the nil "not mine" arm).
    pub fn handler(&self, key: &str) -> Option<rut_vm::OpaqueRef> {
        self.map.get(key).cloned()
    }
}

/// The plugin-state wrapper the rows resolve the table through (stamped by
/// [`TurRpcBusPlugin::register`] — the rows pair with the bus plugin).
#[derive(Clone)]
pub struct RpcHandlersCell(pub std::rc::Rc<std::cell::RefCell<RpcHandlers>>);

/// Resolve the calling instance's sealed-handler table.
fn handlers_of(
    handles: &std::rc::Rc<tur_engine::core::rut_runtime::RutHandles>,
) -> std::rc::Rc<std::cell::RefCell<RpcHandlers>> {
    handles
        .inst
        .plugin_state::<RpcHandlersCell>()
        .map(|c| c.0.clone())
        .expect("rpc bind rows: no RpcHandlersCell in plugin state (the bus plugin is not installed)")
}

/// The bind-table rows' leading-handle decode: the engine-sealed boot box
/// (a rut-value box over `TY_U64`) whose value equals the instance's own
/// [`RpcIdentity`] stamp. Anything else — a foreign id, a script-minted
/// box of another shape — traps `emp:rpc:`.
fn decode_rpc_handle(
    vm: &rut_vm::interp::Vm,
    h: &rut_vm::OpaqueRef,
    handles: &std::rc::Rc<tur_engine::core::rut_runtime::RutHandles>,
) -> Result<(), rut_vm::Trap> {
    let foreign = |why: String| {
        rut_vm::Trap::new(
            rut_vm::TrapKind::Invalid,
            format!("emp:rpc: foreign handle opaque — services take the start(cx) handle ({why})"),
        )
    };
    let (slot, ty) = vm
        .opaque_rut_value(rut_vm::Slot { r: h.ptr() })
        .ok_or_else(|| foreign("the box holds neither the boot identity nor a handle".to_string()))?;
    if ty != rut_core::types::TY_U64 {
        return Err(foreign(format!("sealed value is {ty:?}, not u64")));
    }
    // The seal wrote the i64 bit pattern (`Slot::int`); the id is the same
    // bits read as u64.
    let id = unsafe { slot.i } as u64;
    let stamp = handles
        .inst
        .data::<RpcIdentity>()
        // The host-test lane: `TurRpcBindPlugin::with_identity` stamps the
        // fixed id into the plugin state (no instance-data hook there).
        .or_else(|| {
            handles
                .inst
                .plugin_state::<RpcIdentity>()
                .map(|rc| (*rc).clone())
        })
        .ok_or_else(|| {
            rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                "emp:rpc: no rpc identity bound to this instance (spawned without the emp stamp)"
                    .to_string(),
            )
        })?;
    if id != stamp.0 {
        return Err(foreign(format!("id {id} is not this instance's handle")));
    }
    Ok(())
}

/// The bind-table rows' DECLS — the `rpc` path. `emp_host` extends its own
/// decl list with these (the caller owns the pkg tree — the one-owner
/// rule); [`TurRpcBindPlugin`] mirrors them standalone for host tests.
/// Every scoped row's FIRST parameter is the handle.
pub fn bind_rows_decl() -> Vec<rut_driver::HostRow> {
    use rut_core::types::{TY_NIL, TY_OPAQUE, TY_OPT_OPAQUE, TY_STR};
    let row = |name: &str, params: Vec<rut_core::types::TypeId>, ret: rut_core::types::TypeId| {
        tur_engine::core::rut_runtime::host_row_at("rpc", name, params, ret, false)
    };
    vec![
        // bind(key, f) — seal + file a handler (the key is the op, or the
        // `bin:` / `event:` prefixed form the callers compose).
        row("bind", vec![TY_OPAQUE, TY_STR, TY_OPAQUE], TY_NIL),
        // handler(key) -> ?opaque — the sealed handler back for invocation
        // (nil = not bound).
        row("handler", vec![TY_OPAQUE, TY_STR], TY_OPT_OPAQUE),
    ]
}

/// The bind-table row BODIES under `pkg`'s own name scope
/// (`rpc::bind` / `rpc::handler`) — the relocation seam: `emp_host`'s body
/// pkg calls this exactly once (one owner), the `rpc_host` test pkg
/// mirrors it. The row DECLS stay with the caller (matching full names are
/// verified at boot).
pub fn install_rows(
    pkg: &mut rut_vm::interp::HostPkg,
    handles: &std::rc::Rc<tur_engine::core::rut_runtime::RutHandles>,
) {
    // bind(key, f) — the sealed closure enters the per-instance table.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rpc::bind", (rut_vm::OpaqueRef, &str, rut_vm::OpaqueRef) -> (), move |vm: &mut rut_vm::interp::Vm, hdl: rut_vm::OpaqueRef, key: &str, f: rut_vm::OpaqueRef| {
        decode_rpc_handle(vm, &hdl, &h)?;
        handlers_of(&h).borrow_mut().bind(key, f);
        Ok(())
    });

    // handler(key) -> ?opaque — the sealed handler back for the dispatch
    // entry to downcast + invoke (nil = not bound).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rpc::handler", (rut_vm::OpaqueRef, &str) -> Option<rut_vm::OpaqueRef>, move |vm: &mut rut_vm::interp::Vm, hdl: rut_vm::OpaqueRef, key: &str| {
        decode_rpc_handle(vm, &hdl, &h)?;
        Ok(handlers_of(&h).borrow().handler(key))
    });
}

/// The bind-table rows as a standalone host pkg (`rpc_host`) — the
/// host-test mount: the same [`bind_rows_decl`] + [`install_rows`] bodies
/// `emp_host` carries, offered without the SDK surface so the
/// ease-tur-rpc / ease-js-storage fixtures can bind through the rows
/// directly.
pub fn bind_rows_ext(cx: &mut tur_engine::core::rut_runtime::RutPkgCx<'_>) {
    use rut_driver::Mounted;
    use tur_engine::core::rut_runtime::mount_host_tree;

    let mut m = Mounted::host("rpc_host", bind_rows_decl());
    mount_host_tree(&mut m);
    cx.offered.push(m);

    let Some(handles) = cx.handles else {
        return; // compile-time decl probe — bodies install at boot only
    };
    let mut pkg = rut_vm::interp::HostPkg::new("rpc_host");
    install_rows(&mut pkg, handles);
    cx.boot_pkgs.push(pkg);
}

/// Host-test plugin: installs the [`bind_rows_ext`] mount (`rpc_host`).
/// Pair with [`TurRpcBusPlugin`] (it stamps the
/// [`RpcHandlersCell`] the bodies resolve).
pub struct TurRpcBindPlugin {
    /// The fixed identity stamped into every instance's plugin state (the
    /// host-test lane — production stamps instance data from the minted
    /// handle at spawn).
    identity: Option<u64>,
}

impl TurRpcBindPlugin {
    pub fn new() -> Self {
        Self { identity: None }
    }

    /// Stamp a fixed boot identity ([`RpcIdentity`]) for the rows to
    /// decode against — the host-test shape of the embedder's
    /// instance-data stamp.
    pub fn with_identity(id: u64) -> Self {
        Self { identity: Some(id) }
    }
}

impl Default for TurRpcBindPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl tur_engine::core::plugin::Plugin for TurRpcBindPlugin {
    fn register(
        &self,
        ctx: &mut tur_engine::core::plugin::PluginRegisterContext,
    ) -> Result<(), tur_engine::error::TurError> {
        if let Some(id) = self.identity {
            ctx.define_plugin_state(std::rc::Rc::new(RpcIdentity(id)));
        }
        ctx.push_rut_ext(std::rc::Rc::new(bind_rows_ext));
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The stash-bus state (the old contract — live until phase 5).
// ---------------------------------------------------------------------------

/// Per-instance bus state: the op/event tables + the request inbox.
///
/// Shared three ways: the rut rows (`ease.rpc.*`) reach it through the
/// instance's plugin state / instance data ([`RpcBusCell`]), the embedder
/// stamps it at `createInstance`, and the [`RpcClient`] holds the same
/// `Arc`.
pub struct RpcBus {
    ops: Mutex<HashMap<String, String>>,
    events: Mutex<HashMap<String, Vec<String>>>,
    inbox: Mutex<Option<String>>,
    /// Set once the first `register_op` lands — diagnostics for "the backend
    /// loaded but registered nothing".
    registered: AtomicBool,
}

impl RpcBus {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            ops: Mutex::new(HashMap::new()),
            events: Mutex::new(HashMap::new()),
            inbox: Mutex::new(None),
            registered: AtomicBool::new(false),
        })
    }

    /// `ease.rpc.register(op, export)` — bind a contract op to the entry fn
    /// that serves it. Re-registering an op replaces the binding.
    pub fn register_op(&self, op: &str, export: &str) {
        self.registered.store(true, Ordering::Relaxed);
        self.ops
            .lock()
            .expect("rpc bus ops poisoned")
            .insert(op.to_string(), export.to_string());
    }

    /// `ease.rpc.subscribe_event(type, export)` — append the export to the
    /// event's fan-out list (idempotent per export).
    pub fn subscribe_event(&self, event_type: &str, export: &str) {
        let mut events = self.events.lock().expect("rpc bus events poisoned");
        let list = events.entry(event_type.to_string()).or_default();
        if !list.iter().any(|e| e == export) {
            list.push(export.to_string());
        }
    }

    /// `ease.rpc.take_request()` — drain the inbox ("" when empty).
    pub fn take_request(&self) -> String {
        self.inbox
            .lock()
            .expect("rpc bus inbox poisoned")
            .take()
            .unwrap_or_default()
    }

    /// Look up the export serving `op`.
    pub fn export_for_op(&self, op: &str) -> Option<String> {
        self.ops
            .lock()
            .expect("rpc bus ops poisoned")
            .get(op)
            .cloned()
    }

    /// The exports subscribed to `event_type`.
    pub fn exports_for_event(&self, event_type: &str) -> Vec<String> {
        self.events
            .lock()
            .expect("rpc bus events poisoned")
            .get(event_type)
            .cloned()
            .unwrap_or_default()
    }

    fn stash(&self, json: String) {
        *self.inbox.lock().expect("rpc bus inbox poisoned") = Some(json);
    }

    /// Test/driver seam: enqueue a request payload directly (the same slot
    /// `call_host` fills). The next `take_request()` in the plugin drains
    /// it.
    pub fn stash_request(&self, json: String) {
        self.stash(json);
    }

    /// Whether any op was ever registered (diagnostics).
    pub fn has_registrations(&self) -> bool {
        self.registered.load(Ordering::Relaxed)
    }
}

impl Default for RpcBus {
    fn default() -> Self {
        Self {
            ops: Mutex::new(HashMap::new()),
            events: Mutex::new(HashMap::new()),
            inbox: Mutex::new(None),
            registered: AtomicBool::new(false),
        }
    }
}

/// The plugin-state / instance-data wrapper the rows resolve the bus through.
///
/// A distinct newtype (instead of bare `Arc<RpcBus>`) so the instance-data
/// slot never collides with an embedder's own `Arc<_>` types.
#[derive(Clone)]
pub struct RpcBusCell(pub Arc<RpcBus>);

#[derive(Error, Debug)]
pub enum RpcError {
    /// The backend cannot serve this op — never bound on the bus (old
    /// lane) or no binary entry exported (new lane). Carries the op name.
    #[error("unknown rpc op `{0}` (not registered by the plugin backend)")]
    UnknownOp(String),
    /// The handler failed — the plugin's own failure. New lane: the trap
    /// message (keep the `UNAUTHORIZED` / `TIMEOUT` prefix law); old lane:
    /// the answered error envelope's message.
    #[error("{0}")]
    Handler(String),
    /// The engine hop failed (stale instance, module gone, trap).
    #[error("rpc transport: {0}")]
    Transport(String),
    /// The reply was not a valid payload / envelope.
    #[error("rpc reply decode: {0}")]
    Decode(String),
}

/// The dispatch lanes' answer — the `?str` / `?bytes` entry returns,
/// nil-flattened.
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchAnswer {
    /// The nil answer — "not mine" (the lyric dispatch's fall-through) /
    /// the unsubscribed arm.
    Nil,
    Str(String),
    Bytes(Vec<u8>),
}

/// The engine hop, abstracted so host tests can drive a rut module without
/// the Android runtime. All methods are blocking; impls cross onto the
/// instance's tur-host thread (see the crate docs).
pub trait RutDriver: Send + Sync {
    /// Call `entry fn(opaque) -> str` and return the answered string.
    fn call_entry_str(&self, name: &str, cx: u64) -> Result<String, String>;
    /// Call `entry fn(opaque)` (answer ignored — event dispatch).
    fn call_entry(&self, name: &str, cx: u64) -> Result<(), String>;
    /// The `entry fn start() -> opaque` answer token; 0 until the module
    /// boots (callers treat 0 as "not loaded"). The OLD lane's context
    /// token — the bind-table lane addresses the boot identity the client
    /// already holds ([`RpcClient::new`]'s `start_cx`).
    fn start_token(&self) -> u64;
    /// The str dispatch lane: `entry fn(opaque, str, str)` — the by-name
    /// entry (the module's `rpc` export), `(op, args)` riding the call,
    /// the answer decoded nil-flattened.
    fn call_dispatch_str(
        &self,
        name: &str,
        cx: u64,
        op: &str,
        args: &str,
    ) -> Result<DispatchAnswer, String>;
    /// The binary dispatch lane: `entry fn(opaque, str, bytes)`.
    fn call_dispatch_bytes(
        &self,
        name: &str,
        cx: u64,
        op: &str,
        args: &[u8],
    ) -> Result<DispatchAnswer, String>;
}

/// The per-instance lane verdict (recorded once — the detection point).
#[derive(Debug, Clone, Copy, PartialEq)]
enum Lane {
    /// Neither dispatch entry exported — the stash-bus contract.
    Bus,
    /// The bind-table contract; the entries the module exports.
    Entries { str: bool, bin: bool },
}

impl Lane {
    fn has_str(&self) -> bool {
        matches!(self, Lane::Entries { str: true, .. })
    }
    fn has_bin(&self) -> bool {
        matches!(self, Lane::Entries { bin: true, .. })
    }
    /// Whether ANY dispatch entry exported (the extract gate: a new-style
    /// backend never answers a start token — the entries ARE the boot
    /// proof).
    fn is_entries(&self) -> bool {
        matches!(self, Lane::Entries { .. })
    }
}

/// The client half. `Clone` shares the bus, driver, call gate, boot
/// identity and the recorded lane.
pub struct RpcClient {
    bus: Arc<RpcBus>,
    driver: Arc<dyn RutDriver>,
    /// Serializes call_host/call_host_bytes/emit_event per instance — the
    /// old lane's inbox is single-slot and the entry rail is one VM call
    /// at a time.
    gate: Arc<tokio::sync::Mutex<()>>,
    /// The OLD lane's context token (the start answer), resolved once.
    token: std::sync::OnceLock<u64>,
    /// The NEW lane's context: the embedder-supplied boot identity (the
    /// minted plugin handle id — the host already holds it). 0 = unknown
    /// (the old-lane fixtures) — the bind-table lane can't be addressed
    /// and the probe falls straight to [`Lane::Bus`].
    start_cx: u64,
    /// The capability verdict (the lazy probe records it once).
    lane: std::sync::OnceLock<Lane>,
}

impl Clone for RpcClient {
    fn clone(&self) -> Self {
        Self {
            bus: self.bus.clone(),
            driver: self.driver.clone(),
            gate: self.gate.clone(),
            token: self.token.clone(),
            start_cx: self.start_cx,
            lane: self.lane.clone(),
        }
    }
}

impl RpcClient {
    /// Pair a bus with a driver. `start_cx` is the embedder-supplied boot
    /// identity (the minted plugin handle id — phase 5's identity rail);
    /// old-lane-only fixtures pass 0.
    pub fn new(bus: Arc<RpcBus>, driver: Arc<dyn RutDriver>, start_cx: u64) -> Self {
        Self {
            bus,
            driver,
            gate: Arc::new(tokio::sync::Mutex::new(())),
            token: std::sync::OnceLock::new(),
            start_cx,
            lane: std::sync::OnceLock::new(),
        }
    }

    /// The OLD lane's context token, resolved once (0 = the module never
    /// answered an opaque start — the bus dispatch then fails with
    /// [`RpcError::Transport`]).
    fn token(&self) -> Result<u64, RpcError> {
        match self.token.get() {
            Some(t) => Ok(*t),
            None => {
                let t = self.driver.start_token();
                let _ = self.token.set(t);
                Ok(*self.token.get().unwrap_or(&0))
            }
        }
    }

    /// The capability probe: fire the inert [`PROBE_OP`] at one dispatch
    /// entry. A reply proves the export (the bind table has no handler
    /// under the probe op, so nothing runs); the engine's `no export`
    /// refusal proves its absence; any other failure (worker gone, module
    /// not booted) conservatively reads as absent — the old lane surfaces
    /// the familiar errors. No identity to address with ([`RpcClient::new`]'s
    /// `start_cx == 0`) is an immediate absent.
    fn probe_entry(&self, name: &str, bin: bool) -> bool {
        if self.start_cx == 0 {
            return false;
        }
        let result = if bin {
            self.driver
                .call_dispatch_bytes(name, self.start_cx, PROBE_OP, b"")
        } else {
            self.driver
                .call_dispatch_str(name, self.start_cx, PROBE_OP, "")
        };
        match result {
            // A reply proves the export (the probe op binds nothing —
            // nothing ran).
            Ok(_) => true,
            // The engine's `no export` refusal: definitively absent.
            Err(e) if e.contains(NO_EXPORT_MARKER) => false,
            // Exists-but-trapped ALSO proves the export (the probe op
            // binds nothing, so a trap means the entry's own body is
            // broken — the real dispatch will surface it).
            Err(e) if e.contains(TRAP_SEPARATOR) => true,
            // Anything else is transport (worker gone, module not booted)
            // — conservatively absent: the old lane surfaces the
            // familiar errors.
            Err(_) => false,
        }
    }

    /// The lane verdict, resolved once (the detection point). Eagerly
    /// callable — `extract_rpc` probes up front so the dispatch paths
    /// never pay the probe cost.
    fn detect(&self) -> Lane {
        *self.lane.get_or_init(|| {
            let str_entry = self.probe_entry(DISPATCH_ENTRY, false);
            let bin_entry = self.probe_entry(DISPATCH_ENTRY_BIN, true);
            if !str_entry && !bin_entry {
                Lane::Bus
            } else {
                Lane::Entries {
                    str: str_entry,
                    bin: bin_entry,
                }
            }
        })
    }

    /// The recorded lane's shape (probing on first call) — `extract_rpc`'s
    /// boot gate: a new-style backend answers no start token, the
    /// dispatch entries ARE its boot proof.
    pub fn has_dispatch_entries(&self) -> bool {
        self.detect().is_entries()
    }

    /// The new lane's str dispatch: one by-name entry call with `(op,
    /// args)`; the host builds the envelope from the value/trap (nil ⇒
    /// the null result — "not mine"; trap ⇒ the error arm).
    fn dispatch_str(&self, op: &str, args: &str) -> Result<Value, RpcError> {
        match self
            .driver
            .call_dispatch_str(DISPATCH_ENTRY, self.start_cx, op, args)
            .map_err(rpc_error_of_transport)?
        {
            DispatchAnswer::Nil => Ok(Value::Null),
            DispatchAnswer::Str(s) => serde_json::from_str(&s).map_err(|e| {
                RpcError::Decode(format!(
                    "dispatch reply is not JSON: {e} — `{}`",
                    truncate(&s)
                ))
            }),
            DispatchAnswer::Bytes(_) => Err(RpcError::Decode(
                "str op answered the binary lane".to_string(),
            )),
        }
    }

    /// Resolve `op → export`, stash the payload, fire the entry, decode
    /// the envelope (old lane). `args` rides verbatim as the payload's
    /// `args` field.
    fn call_host_bus(&self, op: &str, args: &Value) -> Result<Value, RpcError> {
        let export = self
            .bus
            .export_for_op(op)
            .ok_or_else(|| RpcError::UnknownOp(op.to_string()))?;
        let token = self.token()?;
        if token == 0 {
            return Err(RpcError::Transport(
                "the plugin module has not booted (no context token)".into(),
            ));
        }
        self.bus.stash(serde_json::json!({ "op": op, "args": args }).to_string());
        let reply = self
            .driver
            .call_entry_str(&export, token)
            .map_err(RpcError::Transport)?;
        decode_envelope(&reply)
    }

    /// Serve a str op — raw JSON args in, the result payload out (`Value::Null`
    /// = the plugin answered "not mine"). Both lanes: the bind-table
    /// dispatch when the module exports the `rpc` entry, the stash bus
    /// otherwise (un-migrated plugins — dual-mode until phase 5).
    pub async fn call_host(&self, op: &str, args: Value) -> Result<Value, RpcError> {
        let _guard = self.gate.lock().await;
        if self.detect().has_str() {
            return self.dispatch_str(op, &args.to_string());
        }
        self.call_host_bus(op, &args)
    }

    /// Serve a bytes op over the binary lane — raw bytes in, raw bytes
    /// out (no base64). `Ok(None)` = the plugin answered "not mine"
    /// (nil); a module without the `rpc_bin` export is an
    /// [`RpcError::UnknownOp`].
    pub async fn call_host_bytes(&self, op: &str, args: Vec<u8>) -> Result<Option<Vec<u8>>, RpcError> {
        let _guard = self.gate.lock().await;
        if !self.detect().has_bin() {
            return Err(RpcError::UnknownOp(op.to_string()));
        }
        match self
            .driver
            .call_dispatch_bytes(DISPATCH_ENTRY_BIN, self.start_cx, op, &args)
            .map_err(rpc_error_of_transport)?
        {
            DispatchAnswer::Nil => Ok(None),
            DispatchAnswer::Bytes(b) => Ok(Some(b)),
            DispatchAnswer::Str(_) => Err(RpcError::Decode(
                "bytes op answered the str lane".to_string(),
            )),
        }
    }

    /// Fire-and-forget event fan-out. Payload errors are logged, never
    /// surfaced (the old event semantics). New lane: ONE dispatch with the
    /// `event:<type>` key (the bind table's subscriber answers, if any,
    /// are ignored); old lane: the per-export stash fan-out.
    pub async fn emit_event(&self, event_type: &str, payload: Value) {
        let _guard = self.gate.lock().await;
        if self.detect().has_str() {
            let key = format!("{EVENT_KEY_PREFIX}{event_type}");
            if let Err(e) = self.driver.call_dispatch_str(
                DISPATCH_ENTRY,
                self.start_cx,
                &key,
                &payload.to_string(),
            ) {
                tracing::warn!(event = event_type, "plugin event dispatch failed: {e}");
            }
            return;
        }
        let exports = self.bus.exports_for_event(event_type);
        if exports.is_empty() {
            return;
        }
        let Ok(token) = self.token() else {
            return;
        };
        if token == 0 {
            return;
        }
        for export in exports {
            self.bus.stash(
                serde_json::json!({ "type": event_type, "payload": payload })
                    .to_string(),
            );
            if let Err(e) = self.driver.call_entry(&export, token) {
                tracing::warn!(event = event_type, export, "plugin event dispatch failed: {e}");
            }
        }
    }

    /// The bus this client drives (diagnostics / tests).
    pub fn bus(&self) -> &Arc<RpcBus> {
        &self.bus
    }

    /// Eagerly resolve the OLD lane's context token (0 = the module has
    /// not booted / answers no opaque start — the new contract's shape).
    pub fn bus_token(&self) -> u64 {
        self.token().unwrap_or(0)
    }
}

/// Decode `{"ok":true,"result":…}` / `{"ok":false,"error":"…"}`.
fn decode_envelope(reply: &str) -> Result<Value, RpcError> {
    let v: Value = serde_json::from_str(reply)
        .map_err(|e| RpcError::Decode(format!("reply is not JSON: {e} — `{reply}`")))?;
    match v.get("ok") {
        Some(Value::Bool(true)) => Ok(v
            .get("result")
            .cloned()
            .ok_or_else(|| RpcError::Decode("ok envelope missing `result`".into()))?),
        Some(Value::Bool(false)) => {
            let msg = v
                .get("error")
                .and_then(|e| e.as_str())
                .unwrap_or("unspecified plugin error");
            Err(RpcError::Handler(msg.to_string()))
        }
        _ => Err(RpcError::Decode(format!(
            "reply envelope missing `ok` — `{}`",
            truncate(reply)
        ))),
    }
}

fn truncate(s: &str) -> &str {
    match s.char_indices().nth(200) {
        Some((i, _)) => &s[..i],
        None => s,
    }
}

/// Build the ok envelope (the view→backend `call` row's pass-through face).
pub fn ok_envelope(result: Value) -> Value {
    serde_json::json!({ "ok": true, "result": result })
}

/// Build the error envelope.
pub fn err_envelope(message: &str) -> Value {
    serde_json::json!({ "ok": false, "error": message })
}

// ---------------------------------------------------------------------------
// The `ease.rpc` host rows (the bus plugin — the OLD contract's rows).
// ---------------------------------------------------------------------------

/// Registers the `ease.rpc` rows the rut side needs to serve the bus:
///
/// - `register(op: str, export: str)` — bind a contract op.
/// - `subscribe_event(type: str, export: str)` — join an event fan-out.
/// - `take_request() -> str` — drain the pending call/event payload ("" = none).
///
/// and stamps the per-instance [`RpcHandlersCell`] the bind-table rows
/// resolve (the two contracts share this plugin: the bus rows retire in
/// phase 5, the sealed-handler cell stays).
///
/// The bus comes from the per-instance data slot (`RpcBusCell`, stamped by
/// the embedder); when absent (host tests), `register` mints a private one
/// into plugin state so the harness flows work unchanged.
pub struct TurRpcBusPlugin {
    /// The shared bus for engine-stamped instances (`None` = mint fresh per
    /// instance — the host-test path).
    bus: Option<Arc<RpcBus>>,
}

impl TurRpcBusPlugin {
    pub fn new(bus: Option<Arc<RpcBus>>) -> Self {
        Self { bus }
    }
}

impl tur_engine::core::plugin::Plugin for TurRpcBusPlugin {
    fn register(&self, ctx: &mut tur_engine::core::plugin::PluginRegisterContext) -> Result<(), tur_engine::error::TurError> {
        let cell = match &self.bus {
            Some(b) => RpcBusCell(b.clone()),
            None => RpcBusCell(RpcBus::new()),
        };
        // Per-instance plugin state: `register` re-runs per instance, so the
        // minted cells are instance-fresh on the test path.
        ctx.define_plugin_state(std::rc::Rc::new(cell));
        ctx.define_plugin_state(std::rc::Rc::new(RpcHandlersCell(std::rc::Rc::new(
            std::cell::RefCell::new(RpcHandlers::default()),
        ))));
        ctx.push_rut_ext(std::rc::Rc::new(install_rpc_rows));
        Ok(())
    }
}

fn install_rpc_rows(cx: &mut tur_engine::core::rut_runtime::RutPkgCx<'_>) {
    use rut_core::types::{TY_NIL, TY_STR};


    cx.decl.extend(
        vec![
            ("register", vec![TY_STR, TY_STR], TY_NIL, false),
            ("subscribe_event", vec![TY_STR, TY_STR], TY_NIL, false),
            ("take_request", vec![], TY_STR, false),
        ]
        .into_iter()
        .map(|(n, p, r, a)| {
            tur_engine::core::rut_runtime::host_row_at("ease/rpc", n, p, r, a)
        }),
    );

    let Some(handles) = cx.handles else {
        return; // compile-time decl probe
    };
    let pkg = &mut *cx.pkg;

    // register(op, export) — bind op → entry fn name.
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ease::rpc::register", (&str, &str) -> (), move |_vm: &mut rut_vm::interp::Vm, op: &str, export: &str| {
        bus_of(&h).register_op(op, export);
        Ok(())
    });

    // subscribe_event(type, export).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ease::rpc::subscribe_event", (&str, &str) -> (), move |_vm: &mut rut_vm::interp::Vm, ty: &str, export: &str| {
        bus_of(&h).subscribe_event(ty, export);
        Ok(())
    });

    // take_request() -> str — the pending call/event payload ("" = none).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "ease::rpc::take_request", () -> String, move |_vm: &mut rut_vm::interp::Vm| {
        Ok(bus_of(&h).take_request())
    });
}

/// Resolve the bus: instance data first (the embedder-stamped slot), then
/// the plugin state (the host-test path minted at register).
fn bus_of(handles: &std::rc::Rc<tur_engine::core::rut_runtime::RutHandles>) -> Arc<RpcBus> {
    handles
        .inst
        .data::<RpcBusCell>()
        .map(|c| c.0)
        .or_else(|| {
            handles
                .inst
                .plugin_state::<RpcBusCell>()
                .map(|c| c.0.clone())
        })
        .expect("rpc bus: no RpcBusCell in instance data or plugin state")
}

#[cfg(test)]
mod envelope_tests {
    use super::*;

    #[test]
    fn envelope_roundtrip() {
        let ok = ok_envelope(serde_json::json!({ "a": 1 }));
        assert_eq!(decode_envelope(&ok.to_string()).unwrap(), serde_json::json!({ "a": 1 }));

        let err = err_envelope("boom");
        match decode_envelope(&err.to_string()) {
            Err(RpcError::Handler(m)) => assert_eq!(m, "boom"),
            other => panic!("expected Handler, got {other:?}"),
        }
    }

    #[test]
    fn envelope_garbage() {
        assert!(matches!(
            decode_envelope("not json"),
            Err(RpcError::Decode(_))
        ));
        assert!(matches!(
            decode_envelope("{\"result\":1}"),
            Err(RpcError::Decode(_))
        ));
    }

    #[test]
    fn trap_messages_split_off_the_engine_decode() {
        // The engine's eval decode wraps a rut trap as
        // `module eval: {entry}: {kind} — {msg}` — the plugin's message is
        // everything after the FIRST separator (it may itself contain one).
        let err = "module eval: rpc: Panic \u{2014} UNAUTHORIZED: expired \u{2014} token old";
        assert_eq!(
            trap_message(err),
            "UNAUTHORIZED: expired \u{2014} token old",
            "the prefix law reads the trap message's head"
        );
        // A non-trap failure carries no separator — verbatim.
        assert_eq!(trap_message("worker gone"), "worker gone");
        // The mapping: trap-shaped → Handler, plain → Transport.
        assert!(matches!(
            rpc_error_of_transport(err.to_string()),
            RpcError::Handler(m) if m == "UNAUTHORIZED: expired \u{2014} token old"
        ));
        assert!(matches!(
            rpc_error_of_transport("worker gone".to_string()),
            RpcError::Transport(_)
        ));
    }
}
