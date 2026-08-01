//! `ease-tur-rpc` — a private tur plugin that wraps tur's bidirectional event
//! bus into a typed **request/response RPC** between the Rust host and the JS
//! realm, so a JS plugin can act as a service (e.g. a storage provider: the
//! host calls `list(dir)` / `get(path)` and awaits the result).
//!
//! tur's event bus is fire-and-forget pub/sub (`Vec<u8>` one-way). This crate
//! layers id-correlation on top:
//!
//! - **Host → JS (request):** `RpcClient::call(op, args)` (Send, awaitable)
//!   enqueues a request; a per-frame subsystem drains the queue and
//!   `eventBus.emit_to_js(JSON {id, op, args})`.
//! - **JS → host (reply):** the `tur:rpc` JS module dispatches incoming bus
//!   messages to handlers registered via `registerHandler(op, fn)`, awaits the
//!   result (sync or async), and `eventBus.send(JSON {id, ok, result|error})`.
//!   A host `on_bus_event` router matches `id` → resolves the caller's
//!   `oneshot`.
//!
//! All cross-thread state flows through `tokio` channels (`mpsc` for requests,
//! `oneshot` for replies); the `Rc`/!Send tur realm stays on the JS thread.
//! The host pumps frames (autonomous `start(driver)` in production, manual
//! `run_frame()` in tests) to drive delivery.
//!
//! Wire-up is two steps:
//! 1. Register `TurRpcPlugin` on the runtime (after `TurStdPlugin`, which owns
//!    the event bus).
//! 2. After spawning an instance, call [`RpcClient::of`] on it (JS thread) to
//!    connect the bus + install the reply router and obtain a Send
//!    [`RpcClient`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::core::subsystem::{Subsystem, SubsystemFlushContext};
use tur_engine::{EventBus, TurApp};
use tur_engine::error::TurError;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum RpcError {
    #[error("JS handler error: {0}")]
    Handler(String),
    #[error("channel closed (instance dropped?)")]
    ChannelClosed,
    #[error("undecodable reply: {0}")]
    BadReply(String),
    #[error("TurRpcPlugin not installed on this instance")]
    NotInstalled,
    #[error("event bus not available (is TurStdPlugin registered?)")]
    NoEventBus,
}

// ---------------------------------------------------------------------------
// Wire format (JSON over the byte event bus)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OutgoingRequest<'a> {
    id: u64,
    op: &'a str,
    args: &'a Value,
}

#[derive(Deserialize)]
struct Reply {
    id: u64,
    ok: bool,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Option<String>,
}

impl Reply {
    fn into_result(self) -> Result<Value, RpcError> {
        if self.ok {
            Ok(self.result)
        } else {
            Err(RpcError::Handler(self.error.unwrap_or_default()))
        }
    }
}

// ---------------------------------------------------------------------------
// Cross-thread request (Send) — caller (tokio) → JS-thread subsystem
// ---------------------------------------------------------------------------

pub struct RpcRequest {
    pub id: u64,
    pub op: String,
    pub args: Value,
    pub reply: oneshot::Sender<Result<Value, RpcError>>,
}

/// Wrapper so the request `Sender` can live in tur's `instance_data` map.
pub struct RequestSender(pub mpsc::Sender<RpcRequest>);

// ---------------------------------------------------------------------------
// Per-instance inner (!Send, JS-thread only)
// ---------------------------------------------------------------------------

pub struct RpcInner {
    /// The event bus, wired in post-build by [`RpcClient::of`]. `None` until then.
    pub bus: RefCell<Option<EventBus>>,
    /// Awaiters for in-flight requests, keyed by id.
    pub pending: RefCell<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>,
    /// Incoming requests from callers; drained each frame by the subsystem.
    pub rx: RefCell<mpsc::Receiver<RpcRequest>>,
}

/// Per-frame drain: takes pending requests from callers and emits each onto the
/// event bus for the JS side to handle. No-ops until the bus is wired in.
struct RpcSubsystem {
    inner: Rc<RpcInner>,
}

impl Subsystem for RpcSubsystem {
    fn flush_pre_layout(&mut self, _cx: &mut SubsystemFlushContext<'_>) {
        let bus = match self.inner.bus.borrow().clone() {
            Some(b) => b,
            None => return,
        };
        loop {
            let req = match self.inner.rx.borrow_mut().try_recv() {
                Ok(r) => r,
                Err(_) => break,
            };
            let payload = serde_json::to_vec(&OutgoingRequest {
                id: req.id,
                op: &req.op,
                args: &req.args,
            })
            .unwrap_or_default();
            self.inner.pending.borrow_mut().insert(req.id, req.reply);
            bus.emit_to_js(payload);
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

/// Install as a runtime plugin (after `TurStdPlugin`). Spawns no JS by itself
/// — call [`RpcClient::of`] on each instance to obtain a caller handle.
pub struct TurRpcPlugin;

impl Plugin for TurRpcPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        let (tx, rx) = mpsc::channel::<RpcRequest>(64);
        let inner = Rc::new(RpcInner {
            bus: RefCell::new(None),
            pending: RefCell::new(HashMap::new()),
            rx: RefCell::new(rx),
        });
        ctx.store_instance_data::<RpcInner>(inner.clone());
        ctx.store_instance_data::<RequestSender>(Rc::new(RequestSender(tx)));
        ctx.register_subsystem(Box::new(RpcSubsystem { inner }));
        ctx.register_js_module("tur:rpc", RPC_JS, Path::new("tur-rpc.mjs"))?;
        tracing::info!("TurRpcPlugin registered tur:rpc");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Caller handle (Send) — usable from any thread (e.g. a tokio playback task)
// ---------------------------------------------------------------------------

/// Send-able handle for invoking JS handlers. Obtain via [`RpcClient::of`].
pub struct RpcClient {
    tx: mpsc::Sender<RpcRequest>,
    next_id: AtomicU64,
}

impl RpcClient {
    /// Wire the RPC layer into an already-built instance and return a caller
    /// handle. Must be called on the instance's JS thread (the bus + instance
    /// data are `!Send`). The returned [`RpcClient`] is `Send` and may be moved
    /// to another thread.
    pub fn of(app: &TurApp) -> Result<Self, RpcError> {
        let inner = app
            .instance_data::<RpcInner>()
            .ok_or(RpcError::NotInstalled)?;
        let sender = app
            .instance_data::<RequestSender>()
            .ok_or(RpcError::NotInstalled)?;
        let bus = EventBus::of(app).ok_or(RpcError::NoEventBus)?;

        // Give the subsystem the bus so it can start emitting.
        *inner.bus.borrow_mut() = Some(bus.clone());

        // Route JS→host replies to the matching awaiter.
        let ri = inner.clone();
        bus.on_bus_event(move |bytes| {
            match serde_json::from_slice::<Reply>(&bytes) {
                Ok(r) => {
                    if let Some(s) = ri.pending.borrow_mut().remove(&r.id) {
                        let _ = s.send(r.into_result());
                    }
                }
                Err(e) => tracing::warn!("tur:rpc: undecodable reply ({e}), dropping"),
            }
        });

        Ok(RpcClient {
            tx: sender.0.clone(),
            next_id: AtomicU64::new(1),
        })
    }

    /// Invoke JS handler `op` with `args` (JSON) and await its result. Errors
    /// thrown by the handler come back as [`RpcError::Handler`].
    pub async fn call(&self, op: &str, args: Value) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (otx, orx) = oneshot::channel();
        self.tx
            .send(RpcRequest {
                id,
                op: op.to_string(),
                args,
                reply: otx,
            })
            .await
            .map_err(|_| RpcError::ChannelClosed)?;
        orx.await.map_err(|_| RpcError::ChannelClosed)?
    }
}

// ---------------------------------------------------------------------------
// tur:rpc JS glue — dispatch + registerHandler
// ---------------------------------------------------------------------------

const RPC_JS: &str = r#"
import { eventBus, encodeUtf8, decodeUtf8 } from "tur:std";

const handlers = new Map();

eventBus.on((payload) => {
  let req;
  try {
    req = JSON.parse(decodeUtf8(payload));
  } catch (e) {
    return; // not a tur:rpc message
  }
  if (!req || typeof req.id === "undefined" || typeof req.op !== "string") return;
  const fn = handlers.get(req.op);
  if (typeof fn !== "function") {
    eventBus.send(encodeUtf8(JSON.stringify({
      id: req.id, ok: false, error: "no handler for op: " + req.op,
    })));
    return;
  }
  Promise.resolve()
    .then(() => fn(req.args))
    .then(
      (result) => eventBus.send(encodeUtf8(JSON.stringify({
        id: req.id, ok: true, result,
      }))),
      (err) => eventBus.send(encodeUtf8(JSON.stringify({
        id: req.id, ok: false,
        error: String((err && err.message) || err),
      }))),
    );
});

export function registerHandler(op, fn) {
  handlers.set(op, fn);
}
"#;
