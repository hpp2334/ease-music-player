//! `ease-tur-rpc` — typed **request/response RPC** plus a **streaming
//! channel** layered on tur's bidirectional event bus, between the Rust host
//! and the JS realm. Lets a JS plugin act as a service (e.g. a storage
//! provider: the host calls `list(dir)`, opens a byte stream for
//! `get(path)`, and awaits OAuth callbacks).
//!
//! tur's event bus (`EventBusHandle`) is a fire-and-forget byte pub/sub,
//! multiplexed by `channel_id` since tur #190: a message targets exactly one
//! channel, and a message to a channel with no handlers is silently dropped
//! (no broadcast). Since tur #181 the threaded backend wires both directions
//! and **self-wakes**: `emit_to_js` enqueues onto the worker (which flushes +
//! fires the JS `on` callbacks registered on that channel), and JS→host bytes
//! are shipped back to main (where the `on_bus_event` handlers registered on
//! that channel fire). No manual pump / `request_paint` / per-call wake is
//! required — the host holds a [`RpcClient`] (`Send + Clone`) and calls it
//! from any thread (e.g. a tokio playback task).
//!
//! Channel layout (each plugin instance owns its own bus, so channels never
//! cross plugins):
//!
//! - [`RPC_CHANNEL_ID`] (0) — the global RPC channel. Two protocols ride it,
//!   distinguished by a leading magic byte (JSON never starts with `0x00`):
//!   - **Control RPC (JSON):** [`RpcClient::call`] enqueues
//!     `{id, op, args}` via `emit_to_js`. The JS dispatcher (`tur:rpc`) runs
//!     the matching `registerHandler` and replies `{id, ok, result|error}`;
//!     the host-side router (an `on_bus_event` handler) matches `id` →
//!     resolves the caller's `oneshot`.
//!   - **Streaming (binary-framed):** [`RpcClient::open_stream`] opens a
//!     stream — the JS opener pushes chunks via `pushChunk` / `endStream` /
//!     `errorStream`, framed over the bus. The host router decodes the frame
//!     and forwards to a per-stream `mpsc::Receiver<StreamChunk>`.
//! - [`EVENT_CHANNEL_ID`] (1) — plugin-only events, host → JS,
//!   fire-and-forget: [`RpcClient::emit_event`] pushes `{type, payload}`;
//!   the JS side dispatches it to the `onEvent(type, …)` registration. No
//!   reply is sent, and a plugin with no registration simply never hears it.
//!
//! Wire-up is two steps:
//! 1. Register [`TurRpcPlugin`] on the runtime (after `TurStdPlugin`, which
//!    owns the event bus + `encodeUtf8`/`decodeUtf8`). This installs the
//!    `tur:rpc` JS dispatcher module.
//! 2. After spawning an instance, call [`RpcClient::wire`] on it — once, on
//!    the instance's own thread (via `with_app`) — to grab the `Send`
//!    `EventBusHandle`, install the reply router, and obtain a [`RpcClient`].

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use tur_engine::core::event_bus::EventBusHandle;
use tur_engine::core::plugin::{Plugin, PluginContext};
use tur_engine::TurApp;
use tur_engine::error::TurError;

// ---------------------------------------------------------------------------
// Bus channel ids (tur #190 multiplexing) — must match the JS constants in
// RPC_JS below.
// ---------------------------------------------------------------------------

/// Global RPC channel: control RPC (JSON) + stream frames (binary). Shared
/// by all RPC callers of one plugin instance's bus.
pub const RPC_CHANNEL_ID: u64 = 0;

/// Plugin-event channel: host → JS fire-and-forget `{type, payload}` JSON
/// (see [`RpcClient::emit_event`]). One bus per plugin instance, so this
/// channel is naturally plugin-private.
pub const EVENT_CHANNEL_ID: u64 = 1;

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
// Cross-thread shared state (Arc<Mutex> — shared between the Send RpcClient
// and the on_bus_event router, which fires on the main thread)
// ---------------------------------------------------------------------------

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, RpcError>>>>>;
type StreamTable = Arc<Mutex<HashMap<u32, mpsc::Sender<StreamChunk>>>>;

// ---------------------------------------------------------------------------
// Plugin — installs the `tur:rpc` JS dispatcher module. No subsystem: the
// engine's event bus is itself the transport (self-waking both ways since #181).
// ---------------------------------------------------------------------------

/// Install as a runtime plugin (after `TurStdPlugin`). Registers the `tur:rpc`
/// JS module (dispatcher + stream pushers). Then call [`RpcClient::wire`] on
/// each instance to obtain a caller handle.
pub struct TurRpcPlugin;

impl Plugin for TurRpcPlugin {
    fn register(&self, ctx: &mut PluginContext<'_>) -> Result<(), TurError> {
        ctx.register_js_module("tur:rpc", RPC_JS, Path::new("tur-rpc.mjs"))?;
        tracing::info!("TurRpcPlugin registered tur:rpc");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Caller handle (Send + Clone) — usable from any thread
// ---------------------------------------------------------------------------

/// Send-able handle for invoking JS handlers and opening byte streams. Obtain
/// via [`RpcClient::wire`]. Cheaply cloneable so multiple backend tasks can
/// share one (id counters are shared via `Arc`).
#[derive(Clone)]
pub struct RpcClient {
    eb: EventBusHandle,
    pending: Pending,
    streams: StreamTable,
    next_req_id: Arc<AtomicU64>,
    next_stream_id: Arc<AtomicU64>,
}

impl RpcClient {
    /// Wire the RPC layer into an already-built instance and return a caller
    /// handle. Must be called on the instance's own thread (via `with_app`) —
    /// `event_bus_handle()` borrows the `Rc`-based `TurApp`. The returned
    /// [`RpcClient`] is `Send + Clone` and may be moved to any thread.
    pub fn wire(app: &TurApp) -> Result<Self, RpcError> {
        let eb = app.event_bus_handle();
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let streams: StreamTable = Arc::new(Mutex::new(HashMap::new()));

        // Route JS→host messages on the RPC channel: stream frames → stream
        // channels; otherwise → JSON control-RPC replies matched by id. Fires
        // on the main thread (MainBackend dispatch of MainMsg::EventBusToHost).
        let (p, s) = (pending.clone(), streams.clone());
        eb.on_bus_event(RPC_CHANNEL_ID, move |bytes| route_incoming(bytes, &p, &s));

        Ok(RpcClient {
            eb,
            pending,
            streams,
            next_req_id: Arc::new(AtomicU64::new(1)),
            next_stream_id: Arc::new(AtomicU64::new(1)),
        })
    }

    /// Invoke JS handler `op` with `args` (JSON) and await its result. Errors
    /// thrown by the handler come back as [`RpcError::Handler`].
    pub async fn call(&self, op: &str, args: Value) -> Result<Value, RpcError> {
        let id = self.next_req_id.fetch_add(1, Ordering::Relaxed);
        let (otx, orx) = oneshot::channel();
        // Register the awaiter BEFORE emitting so a fast (impossible in
        // practice, but defensive) reply is never missed.
        self.pending.lock().unwrap().insert(id, otx);
        let payload = serde_json::to_vec(&OutgoingRequest { id, op, args: &args })
            .unwrap_or_default();
        // Self-waking: emit_to_js enqueues a WorkerMsg → worker flushes → JS
        // dispatcher runs → reply ships back via MainMsg::EventBusToHost →
        // router resolves the oneshot.
        self.eb.emit_to_js(RPC_CHANNEL_ID, payload);
        orx.await.map_err(|_| RpcError::ChannelClosed)?
    }

    /// Fire a plugin event at the JS realm on the dedicated event channel:
    /// pushes `{type, payload}` JSON, delivered to the `onEvent(type, …)`
    /// registration on the next flush. Fire-and-forget — no reply is sent,
    /// and a plugin with no registration silently never hears it (standard
    /// channel semantics since tur #190).
    pub fn emit_event(&self, event_type: &str, payload: Value) {
        let frame = serde_json::json!({ "type": event_type, "payload": payload });
        let bytes = serde_json::to_vec(&frame).unwrap_or_default();
        self.eb.emit_to_js(EVENT_CHANNEL_ID, bytes);
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
        self.streams.lock().unwrap().insert(stream_id, stx);
        // inject the host-assigned stream id into the args object
        if let Some(obj) = args.as_object_mut() {
            obj.insert("streamId".to_string(), Value::from(stream_id));
        }
        let metadata = self.call(op, args).await?;
        Ok((metadata, srx))
    }
}

/// Shared incoming-message router (host `on_bus_event` handler body).
fn route_incoming(bytes: Vec<u8>, pending: &Pending, streams: &StreamTable) {
    let first = match bytes.first() {
        Some(b) => *b,
        None => return,
    };
    match first {
        // stream frame
        MAGIC_CHUNK => {
            if let Some(sid) = read_stream_id(&bytes) {
                let chunk = Bytes::copy_from_slice(&bytes[5..]);
                forward_stream(streams, sid, StreamChunk::Data(chunk));
            }
        }
        MAGIC_END => {
            if let Some(sid) = read_stream_id(&bytes) {
                forward_stream(streams, sid, StreamChunk::End);
            }
        }
        MAGIC_ERR => {
            if let Some(sid) = read_stream_id(&bytes) {
                let msg = String::from_utf8_lossy(&bytes[5..]).into_owned();
                forward_stream(streams, sid, StreamChunk::Error(msg));
            }
        }
        // otherwise: JSON control-RPC reply
        _ => match serde_json::from_slice::<Reply>(&bytes) {
            Ok(r) => {
                if let Some(s) = pending.lock().unwrap().remove(&r.id) {
                    let _ = s.send(r.into_result());
                }
            }
            Err(e) => tracing::warn!("tur:rpc: undecodable reply ({e}), dropping"),
        },
    }
}

/// Deliver a stream chunk to its channel and, on End/Error, drop the sender.
fn forward_stream(streams: &StreamTable, sid: u32, chunk: StreamChunk) {
    let is_terminal = matches!(chunk, StreamChunk::End | StreamChunk::Error(_));
    let mut table = streams.lock().unwrap();
    let sender = if is_terminal {
        table.remove(&sid)
    } else {
        table.get(&sid).cloned()
    };
    drop(table);
    if let Some(sender) = sender {
        // best-effort; a full/closed channel just drops the chunk
        let _ = sender.try_send(chunk);
    }
}

// ---------------------------------------------------------------------------
// tur:rpc JS glue — dispatch, registerHandler, stream pushers
// ---------------------------------------------------------------------------

const RPC_JS: &str = r#"
import { eventBus, encodeUtf8, decodeUtf8 } from "tur:std";

// Bus channel ids — must match the Rust constants RPC_CHANNEL_ID (0) and
// EVENT_CHANNEL_ID (1) in this crate.
const RPC_CH = 0;
const EVENT_CH = 1;

const handlers = new Map();
const eventHandlers = new Map();

// --- control RPC --------------------------------------------------------
eventBus.on(RPC_CH, (payload) => {
  let req;
  try {
    req = JSON.parse(decodeUtf8(payload));
  } catch (e) {
    return; // not a JSON control message (stream frames are binary)
  }
  if (!req || typeof req.id === "undefined" || typeof req.op !== "string") return;
  const fn = handlers.get(req.op);
  if (typeof fn !== "function") {
    eventBus.send(RPC_CH, encodeUtf8(JSON.stringify({
      id: req.id, ok: false, error: "no handler for op: " + req.op,
    })));
    return;
  }
  Promise.resolve()
    .then(() => fn(req.args))
    .then(
      (result) => eventBus.send(RPC_CH, encodeUtf8(JSON.stringify({
        id: req.id, ok: true, result,
      }))),
      (err) => eventBus.send(RPC_CH, encodeUtf8(JSON.stringify({
        id: req.id, ok: false,
        error: String((err && err.message) || err),
      }))),
    );
});

export function registerHandler(op, fn) {
  handlers.set(op, fn);
}

// --- plugin events (fire-and-forget, host → JS) --------------------------
eventBus.on(EVENT_CH, (payload) => {
  let ev;
  try {
    ev = JSON.parse(decodeUtf8(payload));
  } catch (e) {
    return;
  }
  if (!ev || typeof ev.type !== "string") return;
  const fn = eventHandlers.get(ev.type);
  if (typeof fn === "function") fn(ev.payload);
});

export function onEvent(type, fn) {
  eventHandlers.set(type, fn);
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
  eventBus.send(RPC_CH, frameStream(0, streamId, bytes));
}
export function endStream(streamId) {
  eventBus.send(RPC_CH, frameStream(1, streamId, null));
}
export function errorStream(streamId, message) {
  eventBus.send(RPC_CH, frameStream(2, streamId, encodeUtf8(String(message))));
}
"#;
