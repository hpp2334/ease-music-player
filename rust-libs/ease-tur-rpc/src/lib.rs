//! `ease-tur-rpc` — the host⇄rut plugin RPC bridge.
//!
//! The tur engine's JS-era event bus is gone; this crate re-implements the
//! request/response + event transport over the **rut entry rail**, keeping
//! the `RpcClient` surface (`call_host` / `call_host_bytes` / `emit_event`)
//! the backend services already use. There is no injected script module and
//! no JS anywhere: the contract with a plugin backend is a **rut entry-fn
//! convention** — the bind table (phase 5 made it the ONLY contract; the
//! old stash-bus lane is deleted).
//!
//! # The bind-table contract (`emp`'s `Rpc` class)
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
//! # Lane capability
//!
//! [`RpcClient`] probes the two dispatch entries ONCE (eagerly, at
//! `extract_rpc` — the detection point is recorded on the client): an
//! inert probe op fires at each entry — a reply (typically the nil
//! "not bound" arm) proves the export, the engine's `no export` refusal
//! proves its absence. [`RpcClient::has_dispatch_entries`] is the boot
//! gate: a backend that exports neither entry has no bind table (the
//! module failed to boot / is not a backend) and extracts no client. The
//! probe op's name is RESERVED at bind time — a handler bound under it
//! would make the capability fire run plugin code.
//!
//! # Streaming (chunked pull)
//!
//! There are no credit-gated push streams. The storage contract
//! (`ease-js-storage`) splits the old `storage:get` opener into three
//! plain ops driven by the host: `storage:get` (opener → `{sid, meta}`),
//! `storage:get-chunk` (`{sid, offset, len}` → `{data: <base64>, eof}`),
//! `storage:release` (`{sid}`). Pull = natural backpressure; str ops keep
//! the base64 crossing (the binary lane exists for the ops that want raw
//! bytes, e.g. `lyric:parse`).
//!
//! # Threading
//!
//! [`RpcClient`] is `Clone` + `Send`; every call serializes through a
//! shared gate (the entry rail is one VM call at a time) and crosses onto
//! the instance's tur-host thread via the [`RutDriver`] impl (a blocking
//! round-trip, the same pattern tur's own `loadModule` uses). Never call
//! back into the *same* instance from inside a dispatch (entry
//! re-entrancy). The sealed closures in [`RpcHandlers`] are `!Send` on
//! purpose — they are VM-heap cells and never leave the instance's thread.
//!
//! # Testability
//!
//! [`RutDriver`] abstracts the engine hop, so host tests drive a real rut
//! module through the plugins + `TurTestApp` without Android.
//! [`TurRpcBindPlugin`] mounts the bind-table rows standalone (the same
//! bodies `emp_host` installs) for fixtures that exercise the lane
//! without the full SDK surface.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
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
/// The capability probe's op — inert by construction: the bind table
/// refuses to bind anything under it (the reservation in
/// [`install_rows`]), so a probe dispatch answers the nil "not bound" arm
/// without running any handler. Reserved namespace.
const PROBE_OP: &str = "__tur_rpc_probe";
/// The engine's missing-export refusal (`RutRuntime::export_ret`'s error,
/// verbatim) — the capability probe's discriminator.
const NO_EXPORT_MARKER: &str = "no export";
/// The engine's trap decode appends `{entry}: {kind}` before the trap
/// message after this separator — [`trap_message`] splits it back off.
const TRAP_SEPARATOR: &str = " \u{2014} "; // " — "

/// Whether a bind key collides with the capability probe (the bare probe
/// op on either lane — the bin lane composes the [`BIN_KEY_PREFIX`] form).
fn is_probe_key(key: &str) -> bool {
    key == PROBE_OP || key == format!("{BIN_KEY_PREFIX}{PROBE_OP}")
}

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

/// Map a dispatch failure onto the client error: a rut trap is the
/// plugin's own failure ([`RpcError::Handler`] — the envelope error arm),
/// anything else is transport.
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
/// [`TurRpcBindPlugin::register`] — the rows pair with the bind plugin).
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
        .expect("rpc bind rows: no RpcHandlersCell in plugin state (the bind plugin is not installed)")
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
    // The probe op's name is RESERVED (phase-5 cleanup): the capability
    // probe dispatches it expecting the inert nil arm — a handler bound
    // under it would run plugin code on every capability check. The key
    // check runs before the identity decode (a key-space law, not an
    // identity one).
    let h = handles.clone();
    rut_vm::pkg_fn!(pkg, "rpc::bind", (rut_vm::OpaqueRef, &str, rut_vm::OpaqueRef) -> (), move |vm: &mut rut_vm::interp::Vm, hdl: rut_vm::OpaqueRef, key: &str, f: rut_vm::OpaqueRef| {
        if is_probe_key(key) {
            return Err(rut_vm::Trap::new(
                rut_vm::TrapKind::Invalid,
                format!("emp:rpc: `{key}` is a reserved probe key — bind your ops under their own names"),
            ));
        }
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

/// Host-test plugin: installs the [`bind_rows_ext`] mount (`rpc_host`) and
/// stamps the [`RpcHandlersCell`] every bind-row body resolves — the same
/// stamps production installs through `emp_host` (the `EaseMusicPlugin`
/// set). `with_identity` additionally stamps a fixed [`RpcIdentity`] (the
/// host-test shape of the embedder's instance-data stamp; production
/// stamps it per instance from the minted handle).
pub struct TurRpcBindPlugin {
    /// The fixed identity stamped into every instance's plugin state
    /// (`None` = production shape — the embedder stamps instance data).
    identity: Option<u64>,
}

impl TurRpcBindPlugin {
    pub fn new() -> Self {
        Self { identity: None }
    }

    /// Stamp a fixed boot identity ([`RpcIdentity`]) for the rows to
    /// decode against.
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
        ctx.define_plugin_state(std::rc::Rc::new(RpcHandlersCell(std::rc::Rc::new(
            std::cell::RefCell::new(RpcHandlers::default()),
        ))));
        ctx.push_rut_ext(std::rc::Rc::new(bind_rows_ext));
        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum RpcError {
    /// The backend cannot serve this op — the binary lane's entry is not
    /// exported (a str-only backend asked for bytes).
    #[error("unknown rpc op `{0}` (not registered by the plugin backend)")]
    UnknownOp(String),
    /// The handler failed — the plugin's own failure: the trap message
    /// (keep the `UNAUTHORIZED` / `TIMEOUT` prefix law).
    #[error("{0}")]
    Handler(String),
    /// The engine hop failed (stale instance, module gone, trap).
    #[error("rpc transport: {0}")]
    Transport(String),
    /// The reply was not a valid payload.
    #[error("rpc reply decode: {0}")]
    Decode(String),
}

/// The dispatch lanes' answer — the `?str` / `?bytes` entry returns,
/// nil-flattened.
#[derive(Debug, Clone, PartialEq)]
pub enum DispatchAnswer {
    /// The nil answer — "not mine" (the lyric dispatch's fall-through) /
    /// the unsubscribed arm / nothing bound under the op.
    Nil,
    Str(String),
    Bytes(Vec<u8>),
}

/// The engine hop, abstracted so host tests can drive a rut module without
/// the Android runtime. All methods are blocking; impls cross onto the
/// instance's tur-host thread (see the crate docs).
pub trait RutDriver: Send + Sync {
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

/// The per-instance lane capability (recorded once — the detection
/// point): which of the two dispatch entries the module exports.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Lane {
    Entries { str: bool, bin: bool },
}

impl Lane {
    fn has_str(&self) -> bool {
        matches!(self, Lane::Entries { str: true, .. })
    }
    fn has_bin(&self) -> bool {
        matches!(self, Lane::Entries { bin: true, .. })
    }
    fn is_entries(&self) -> bool {
        matches!(self, Lane::Entries { .. })
    }
}

/// The client half. `Clone` shares the driver, call gate, boot identity
/// and the recorded lane capability.
pub struct RpcClient {
    driver: Arc<dyn RutDriver>,
    /// Serializes call_host/call_host_bytes/emit_event per instance — the
    /// entry rail is one VM call at a time.
    gate: Arc<tokio::sync::Mutex<()>>,
    /// The boot identity: the embedder-supplied minted plugin handle id.
    /// 0 = unknown — the bind-table lane can't be addressed and the probe
    /// records no entries (the boot gate then refuses the client).
    start_cx: u64,
    /// The capability verdict (the probe records it once).
    lane: std::sync::OnceLock<Lane>,
}

impl Clone for RpcClient {
    fn clone(&self) -> Self {
        Self {
            driver: self.driver.clone(),
            gate: self.gate.clone(),
            start_cx: self.start_cx,
            lane: self.lane.clone(),
        }
    }
}

impl RpcClient {
    /// Pair a driver with the boot identity — the embedder-supplied minted
    /// plugin handle id (the same value the instance's `RpcIdentity` stamp
    /// carries; the bind rows decode it, the dispatch entries address it).
    pub fn new(driver: Arc<dyn RutDriver>, start_cx: u64) -> Self {
        Self {
            driver,
            gate: Arc::new(tokio::sync::Mutex::new(())),
            start_cx,
            lane: std::sync::OnceLock::new(),
        }
    }

    /// The capability probe: fire the inert [`PROBE_OP`] at one dispatch
    /// entry. A reply proves the export (the bind table refuses the probe
    /// key, so nothing runs); the engine's `no export` refusal proves its
    /// absence; exists-but-trapped ALSO proves the export (the probe op
    /// binds nothing, so a trap means the entry's own body is broken —
    /// the real dispatch will surface it). Any other failure (worker
    /// gone, module not booted) conservatively reads as absent. No
    /// identity to address with ([`RpcClient::new`]'s `start_cx == 0`) is
    /// an immediate absent.
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
            // Exists-but-trapped ALSO proves the export.
            Err(e) if e.contains(TRAP_SEPARATOR) => true,
            // Anything else is transport (worker gone, module not booted)
            // — conservatively absent.
            Err(_) => false,
        }
    }

    /// The lane capability, resolved once (the detection point). Eagerly
    /// callable — `extract_rpc` probes up front so the dispatch paths
    /// never pay the probe cost.
    fn detect(&self) -> Lane {
        *self.lane.get_or_init(|| {
            let str_entry = self.probe_entry(DISPATCH_ENTRY, false);
            let bin_entry = self.probe_entry(DISPATCH_ENTRY_BIN, true);
            Lane::Entries {
                str: str_entry,
                bin: bin_entry,
            }
        })
    }

    /// The recorded capability (probing on first call) — `extract_rpc`'s
    /// boot gate: a backend must export at least one dispatch entry (the
    /// bind table IS the boot proof; the module answered no start token).
    pub fn has_dispatch_entries(&self) -> bool {
        self.detect().is_entries()
    }

    /// The str dispatch: one by-name entry call with `(op, args)`; the
    /// host builds the envelope from the value/trap (nil ⇒ the null
    /// result — "not mine"; trap ⇒ the error arm).
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

    /// Serve a str op — raw JSON args in, the result payload out
    /// (`Value::Null` = the plugin answered "not mine"). A nil answer
    /// with no binding behind it is worth a warn (the phase-5 cleanup):
    /// contract ops answer real payloads, so nil usually means the op
    /// was never bound (or a view called a name the backend doesn't
    /// serve) — the legible "not mine" fall-throughs ride the bytes lane
    /// ([`RpcClient::call_host_bytes`]) and stay silent.
    pub async fn call_host(&self, op: &str, args: Value) -> Result<Value, RpcError> {
        let _guard = self.gate.lock().await;
        let value = self.dispatch_str(op, &args.to_string())?;
        if value.is_null() {
            tracing::warn!("rpc dispatch `{op}` answered nil (nothing bound under the op / not-mine)");
        }
        Ok(value)
    }

    /// Serve a bytes op over the binary lane — raw bytes in, raw bytes
    /// out (no base64). `Ok(None)` = the plugin answered "not mine"
    /// (nil; the lyric dispatch's fall-through — silent by design); a
    /// module without the `rpc_bin` export is an [`RpcError::UnknownOp`].
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

    /// Fire-and-forget event fan-out: ONE dispatch with the `event:<type>`
    /// key (the bind table's subscriber answers, if any, are ignored).
    /// Payload errors are logged, never surfaced.
    pub async fn emit_event(&self, event_type: &str, payload: Value) {
        let _guard = self.gate.lock().await;
        if !self.detect().has_str() {
            tracing::warn!(event = event_type, "plugin backend exports no `{DISPATCH_ENTRY}` entry — event dropped");
            return;
        }
        let key = format!("{EVENT_KEY_PREFIX}{event_type}");
        if let Err(e) = self.driver.call_dispatch_str(
            DISPATCH_ENTRY,
            self.start_cx,
            &key,
            &payload.to_string(),
        ) {
            tracing::warn!(event = event_type, "plugin event dispatch failed: {e}");
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_builders_shape_the_pass_through() {
        let ok = ok_envelope(serde_json::json!({ "a": 1 }));
        assert_eq!(ok["ok"], serde_json::json!(true));
        assert_eq!(ok["result"], serde_json::json!({ "a": 1 }));

        let err = err_envelope("boom");
        assert_eq!(err["ok"], serde_json::json!(false));
        assert_eq!(err["error"], serde_json::json!("boom"));
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

    #[test]
    fn the_probe_keys_are_reserved_both_lanes() {
        assert!(is_probe_key("__tur_rpc_probe"));
        assert!(is_probe_key("bin:__tur_rpc_probe"));
        assert!(!is_probe_key("storage:list"));
        assert!(!is_probe_key("bin:lyric:parse"));
        assert!(!is_probe_key("event:music:play"));
    }
}
