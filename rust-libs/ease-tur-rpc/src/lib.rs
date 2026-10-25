//! `ease-tur-rpc` — the host⇄rut plugin RPC bridge.
//!
//! The tur engine's JS-era event bus is gone; this crate re-implements the
//! request/response + event transport over the **rut entry rail**, keeping the
//! `RpcClient` surface (`call_host` / `emit_event`) the backend services
//! already use. There is no injected script module and no JS anywhere: the
//! contract with a plugin backend is a **rut entry-fn convention**.
//!
//! # Architecture (the bus pattern)
//!
//! Each plugin backend instance carries a per-instance [`RpcBus`] (stamped as
//! instance data by the embedder at `createInstance`; read by the host rows
//! through the instance context):
//!
//! - `ops: op → rut export name` — the backend registers each contract op as
//!   its own `entry fn` (`ease.rpc.register("storage:list", "op_storage_list")`),
//!   so dispatch is a compile-checked export call, never an in-module JSON
//!   string switch.
//! - `events: type → [export names]` — `ease.rpc.subscribe_event`.
//! - `inbox: single pending request` — the host stashes the call payload
//!   before firing the entry (the engine's crossing lanes carry no str
//!   argument; the rut side pulls the payload with `ease.rpc.take_request()`).
//!
//! `RpcClient::call_host(op, args)`:
//! 1. resolve `op` → export name (`UnknownOp` when the backend never
//!    registered it),
//! 2. stash `{"op":…,"args":…}` in the inbox,
//! 3. `call_rut_entry_cx_str(export, ctx_token)` — the entry returns the
//!    reply envelope string,
//! 4. parse `{"ok":true,"result":…}` / `{"ok":false,"error":"…"}`.
//!
//! Events ride the same inbox: `emit_event` stashes
//! `{"type":…,"payload":…}` and fires each subscribed export (fire-and-
//! forget).
//!
//! # Streaming (chunked pull)
//!
//! There are no credit-gated push streams anymore. The storage contract
//! (`ease-js-storage`) splits the old `storage:get` opener into three plain
//! ops driven by the host: `storage:get` (opener → `{sid, meta}`),
//! `storage:get-chunk` (`{sid, offset, len}` → `{data: <base64>, eof}`) and
//! `storage:release` (`{sid}`). Pull = natural backpressure; the reply is a
//! str envelope, binary rides base64.
//!
//! # Threading
//!
//! [`RpcClient`] is `Clone` + `Send`; every call serializes through a shared
//! gate (the inbox is single-slot) and crosses onto the instance's tur-host
//! thread via the [`RutDriver`] impl (a blocking round-trip, the same pattern
//! tur's own `loadModule` uses). Never call back into the *same* instance
//! from inside a dispatch (single-slot inbox + face re-entrancy).
//!
//! # Testability
//!
//! [`RutDriver`] abstracts the engine hop, so host tests drive a real rut
//! module through [`TurRpcBusPlugin`] + `TurTestApp` without Android.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use thiserror::Error;

/// Per-instance bus state: the op/event tables + the request inbox.
///
/// Shared three ways: the rut rows (`ease.rpc.*`) reach it through the
/// instance's plugin state / instance data ([`RpcBusCell`]), the embedder
/// stamps it at `createInstance`, and the [`RpcClient`] holds the same `Arc`.
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
    /// The backend never registered this op (or the module is a view-only
    /// load). Carries the op name.
    #[error("unknown rpc op `{0}` (not registered by the plugin backend)")]
    UnknownOp(String),
    /// The entry answered an error envelope — the plugin's own failure.
    #[error("{0}")]
    Handler(String),
    /// The engine hop failed (stale instance, module gone, trap).
    #[error("rpc transport: {0}")]
    Transport(String),
    /// The reply was not a valid envelope.
    #[error("rpc reply decode: {0}")]
    Decode(String),
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
    /// boots (callers treat 0 as "not loaded").
    fn start_token(&self) -> u64;
}

/// The client half. `Clone` shares the bus, driver and the call gate.
pub struct RpcClient {
    bus: Arc<RpcBus>,
    driver: Arc<dyn RutDriver>,
    /// Serializes call_host/emit_event per instance — the inbox is
    /// single-slot and the entry rail is one VM call at a time.
    gate: Arc<tokio::sync::Mutex<()>>,
    token: std::sync::OnceLock<u64>,
}

impl Clone for RpcClient {
    fn clone(&self) -> Self {
        Self {
            bus: self.bus.clone(),
            driver: self.driver.clone(),
            gate: self.gate.clone(),
            token: self.token.clone(),
        }
    }
}

impl RpcClient {
    pub fn new(bus: Arc<RpcBus>, driver: Arc<dyn RutDriver>) -> Self {
        Self {
            bus,
            driver,
            gate: Arc::new(tokio::sync::Mutex::new(())),
            token: std::sync::OnceLock::new(),
        }
    }

    /// The context token, resolved once (0 = the module never answered an
    /// opaque start — dispatch then fails with [`RpcError::Transport`]).
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

    /// Resolve `op → export`, stash the payload, fire the entry, decode the
    /// envelope. `args` rides verbatim as the payload's `args` field.
    pub async fn call_host(&self, op: &str, args: Value) -> Result<Value, RpcError> {
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
        let _guard = self.gate.lock().await;
        self.bus.stash(
            serde_json::json!({ "op": op, "args": args }).to_string(),
        );
        let reply = self
            .driver
            .call_entry_str(&export, token)
            .map_err(RpcError::Transport)?;
        decode_envelope(&reply)
    }

    /// Fire-and-forget event fan-out. Payload errors are logged, never
    /// surfaced (the old event semantics).
    pub async fn emit_event(&self, event_type: &str, payload: Value) {
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
        let _guard = self.gate.lock().await;
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

    /// Eagerly resolve the context token (0 = the module has not booted).
    /// The engine's `extract_rpc` gates on this — a backend whose module
    /// failed to boot never produces a client.
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

/// Build the ok envelope (mirrors what the rut side hand-builds).
pub fn ok_envelope(result: Value) -> Value {
    serde_json::json!({ "ok": true, "result": result })
}

/// Build the error envelope.
pub fn err_envelope(message: &str) -> Value {
    serde_json::json!({ "ok": false, "error": message })
}

// ---------------------------------------------------------------------------
// The `ease.rpc` host rows (the bus plugin).
// ---------------------------------------------------------------------------

/// Registers the `ease.rpc` rows the rut side needs to serve the bus:
///
/// - `register(op: str, export: str)` — bind a contract op.
/// - `subscribe_event(type: str, export: str)` — join an event fan-out.
/// - `take_request() -> str` — drain the pending call/event payload ("" = none).
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
        // minted cell is instance-fresh on the test path.
        ctx.define_plugin_state(std::rc::Rc::new(cell));
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
}
