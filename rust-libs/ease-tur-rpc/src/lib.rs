//! `ease-tur-rpc` — a private tur plugin that wraps tur's bidirectional event
//! bus into a typed **request/response RPC** plus a **streaming channel**
//! between the Rust host and the JS realm, so a JS plugin can act as a service
//! (e.g. a storage provider: the host calls `list(dir)`, opens a byte stream
//! for `get(path)`, and awaits OAuth callbacks).
//!
//! tur's event bus is fire-and-forget pub/sub (`Vec<u8>` one-way). This crate
//! layers two protocols on top, both riding the bus:
//!
//! - **Control RPC (JSON):** `RpcClient::call(op, args)` (Send, awaitable)
//!   enqueues a request; a per-frame subsystem drains it and
//!   `eventBus.emit_to_js(JSON {id, op, args})`. The JS side dispatches to the
//!   matching `registerHandler` and replies `{id, ok, result|error}`; a host
//!   `on_bus_event` router matches `id` → resolves the caller's `oneshot`.
//!
//! - **Streaming (binary-framed):** `RpcClient::open_stream(op, args)` opens a
//!   stream — the JS opener pushes chunks via `pushChunk(streamId, bytes)` /
//!   `endStream` / `errorStream`, which frame+send over the bus. The host
//!   router decodes the frame and forwards to a per-stream
//!   `mpsc::Receiver<StreamChunk>`. JSON replies and stream frames are
//!   distinguished by a leading magic byte (JSON never starts with `0x00`).
//!
//! All cross-thread state flows through `tokio` channels (`mpsc`/`oneshot`) and
//! an `Arc<Mutex>` stream table; the `Rc`/!Send tur realm stays on the JS
//! thread. The host pumps frames (autonomous `start(driver)` in production,
//! manual `run_frame()` in tests) to drive delivery.
//!
//! Wire-up is two steps:
//! 1. Register `TurRpcPlugin` on the runtime (after `TurStdPlugin`, which owns
//!    the event bus).
//! 2. After spawning an instance, call [`RpcClient::of`] on it (JS thread) to
//!    connect the bus + install the routers and obtain a Send [`RpcClient`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
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
// Control-RPC wire format (JSON over the byte event bus)
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
// Streaming wire format (binary frames over the byte event bus)
// ---------------------------------------------------------------------------
//
// A stream frame starts with a magic byte that JSON can never produce (JSON
// begins with `{` 0x7B, `[`, `"`, digit, `-`, `t`/`f`/`n`), so the host router
// can tell a stream frame from a JSON reply by inspecting byte[0]:
//   0x00 chunk  : [0x00][streamId u32 LE][raw bytes...]
//   0x01 end    : [0x01][streamId u32 LE]
//   0x02 error  : [0x02][streamId u32 LE][utf-8 message...]

const MAGIC_CHUNK: u8 = 0x00;
const MAGIC_END: u8 = 0x01;
const MAGIC_ERR: u8 = 0x02;

/// One item pushed by the JS opener of a stream.
#[derive(Debug)]
pub enum StreamChunk {
    Data(Bytes),
    End,
    Error(String),
}

fn read_stream_id(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 5 {
        return None;
    }
    Some(u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]))
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
// Per-instance inner. Mostly !Send (JS-thread only); the stream table is
// Arc<Mutex> so callers on tokio can register streams and the JS-thread router
// can feed them.
// ---------------------------------------------------------------------------

type StreamTable = Arc<Mutex<HashMap<u32, mpsc::Sender<StreamChunk>>>>;

pub struct RpcInner {
    /// The event bus, wired in post-build by [`RpcClient::of`]. `None` until then.
    pub bus: RefCell<Option<EventBus>>,
    /// Awaiters for in-flight control-RPC requests, keyed by id.
    pub pending: RefCell<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>,
    /// Incoming requests from callers; drained each frame by the subsystem.
    pub rx: RefCell<mpsc::Receiver<RpcRequest>>,
    /// Open streams' sender halves, keyed by stream id (host-assigned).
    pub streams: StreamTable,
}

/// Per-frame drain: takes pending control-RPC requests from callers and emits
/// each onto the event bus for the JS side to handle. No-ops until the bus is
/// wired in.
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
            streams: Arc::new(Mutex::new(HashMap::new())),
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

/// Send-able handle for invoking JS handlers and opening byte streams. Obtain
/// via [`RpcClient::of`]. Cheaply cloneable so multiple backend tasks can share
/// one (id counters are shared via `Arc`).
#[derive(Clone)]
pub struct RpcClient {
    tx: mpsc::Sender<RpcRequest>,
    streams: StreamTable,
    next_req_id: Arc<AtomicU64>,
    next_stream_id: Arc<AtomicU64>,
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

        // Route JS→host messages: stream frames → stream channels; otherwise →
        // JSON control-RPC replies matched by id.
        let ri = inner.clone();
        bus.on_bus_event(move |bytes| route_incoming(&ri, bytes));

        Ok(RpcClient {
            tx: sender.0.clone(),
            streams: inner.streams.clone(),
            next_req_id: Arc::new(AtomicU64::new(1)),
            next_stream_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Invoke JS handler `op` with `args` (JSON) and await its result. Errors
    /// thrown by the handler come back as [`RpcError::Handler`].
    pub async fn call(&self, op: &str, args: Value) -> Result<Value, RpcError> {
        let id = self.next_req_id.fetch_add(1, Ordering::Relaxed);
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

    /// Open a byte stream: calls JS handler `op` with `{ streamId, ...args }`,
    /// awaits its metadata reply, and returns the metadata plus a receiver for
    /// the chunks the JS side pushes via `pushChunk` / `endStream` /
    /// `errorStream`. The stream id is host-assigned and passed in `args` under
    /// the `streamId` key (a number).
    pub async fn open_stream(
        &self,
        op: &str,
        mut args: Value,
    ) -> Result<(Value, mpsc::Receiver<StreamChunk>), RpcError> {
        let stream_id = self.next_stream_id.fetch_add(1, Ordering::Relaxed) as u32;
        let (stx, srx) = mpsc::channel::<StreamChunk>(32);
        self.streams
            .lock()
            .unwrap()
            .insert(stream_id, stx);
        // inject the host-assigned stream id into the args object
        if let Some(obj) = args.as_object_mut() {
            obj.insert("streamId".to_string(), Value::from(stream_id));
        }
        let metadata = self.call(op, args).await?;
        Ok((metadata, srx))
    }
}

/// Shared incoming-message router (host `on_bus_event` handler).
fn route_incoming(inner: &Rc<RpcInner>, bytes: Vec<u8>) {
    let first = match bytes.first() {
        Some(b) => *b,
        None => return,
    };
    match first {
        // stream frame
        MAGIC_CHUNK => {
            if let Some(sid) = read_stream_id(&bytes) {
                let chunk = Bytes::copy_from_slice(&bytes[5..]);
                forward_stream(inner, sid, StreamChunk::Data(chunk));
            }
        }
        MAGIC_END => {
            if let Some(sid) = read_stream_id(&bytes) {
                forward_stream(inner, sid, StreamChunk::End);
            }
        }
        MAGIC_ERR => {
            if let Some(sid) = read_stream_id(&bytes) {
                let msg = String::from_utf8_lossy(&bytes[5..]).into_owned();
                forward_stream(inner, sid, StreamChunk::Error(msg));
            }
        }
        // otherwise: JSON control-RPC reply
        _ => match serde_json::from_slice::<Reply>(&bytes) {
            Ok(r) => {
                if let Some(s) = inner.pending.borrow_mut().remove(&r.id) {
                    let _ = s.send(r.into_result());
                }
            }
            Err(e) => tracing::warn!("tur:rpc: undecodable reply ({e}), dropping"),
        },
    }
}

/// Deliver a stream chunk to its channel and, on End/Error, drop the sender.
fn forward_stream(inner: &Rc<RpcInner>, sid: u32, chunk: StreamChunk) {
    let is_terminal = matches!(chunk, StreamChunk::End | StreamChunk::Error(_));
    let mut table = inner.streams.lock().unwrap();
    if let Some(sender) = if is_terminal { table.remove(&sid) } else { table.get(&sid).cloned() } {
        // best-effort; a full/closed channel just drops the chunk
        let _ = sender.try_send(chunk);
    }
}

// ---------------------------------------------------------------------------
// tur:rpc JS glue — dispatch, registerHandler, stream pushers
// ---------------------------------------------------------------------------

const RPC_JS: &str = r#"
import { eventBus, encodeUtf8, decodeUtf8 } from "tur:std";

const handlers = new Map();

// --- control RPC --------------------------------------------------------
eventBus.on((payload) => {
  let req;
  try {
    req = JSON.parse(decodeUtf8(payload));
  } catch (e) {
    return; // not a JSON control message (stream frames are binary)
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

// --- streaming ----------------------------------------------------------
// Binary framing: [magic u8][streamId u32 LE][...payload]. Magic values
// must match the Rust constants (0=chunk, 1=end, 2=error).
function frameStream(kind, streamId, payload) {
  const bodyLen = payload ? payload.length : 0;
  const out = new Uint8Array(5 + bodyLen);
  out[0] = kind;
  const dv = new DataView(out.buffer);
  dv.setUint32(1, streamId >>> 0, true); // little-endian
  if (payload) out.set(payload, 5);
  return out;
}

export function pushChunk(streamId, bytes) {
  eventBus.send(frameStream(0, streamId, bytes));
}
export function endStream(streamId) {
  eventBus.send(frameStream(1, streamId, null));
}
export function errorStream(streamId, message) {
  eventBus.send(frameStream(2, streamId, encodeUtf8(String(message))));
}
"#;
