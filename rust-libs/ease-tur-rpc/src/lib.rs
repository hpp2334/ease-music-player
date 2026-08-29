//! `ease-tur-rpc` — typed **request/response RPC** plus **credit-gated
//! streaming** between the Rust host and the JS plugin realm, layered on
//! tur's bidirectional event bus. Lets a JS plugin act as a service (e.g. a
//! storage provider: the host calls `list(dir)`, opens a byte stream for
//! `get(path, offset)`, and awaits OAuth callbacks).
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
//!   - **Control RPC (JSON):** [`RpcClient::call_host`] /
//!     [`RpcClient::call_view`] enqueue `{id, op, scope, args}` via
//!     `emit_to_js`. The JS dispatcher (`tur:rpc`) routes by scope —
//!     `hostRpc.registerHandler` / `viewRpc.registerHandler` (or a
//!     `hostRpc.registerStream` opener — see below) — and replies
//!     `{id, ok, result|error}`; the host-side router (an `on_bus_event`
//!     handler) matches `id` → resolves the caller's `oneshot`.
//!   - **Streaming (binary-framed):** [`RpcClient::open_stream`] opens a
//!     stream. The JS opener (registered via `hostRpc.registerStream`)
//!     resolves
//!     `{ meta, body, release?, mapError? }` — `meta` becomes the RPC reply,
//!     then the dispatcher pumps `body` through a sid-bound **`StreamProducer`**
//!     (`push`/`end`/`error`) that awaits host-granted credits before each
//!     chunk. Frames over the bus:
//!     `0x00 chunk : [0x00][streamId u32 LE][raw bytes...]`,
//!     `0x01 end    : [0x01][streamId u32 LE]`,
//!     `0x02 error  : [0x02][streamId u32 LE][utf-8 message...]`.
//! - [`EVENT_CHANNEL_ID`] (1) — plugin-only events, host → JS,
//!   fire-and-forget: [`RpcClient::emit_event`] pushes `{type, payload}`;
//!   the JS side dispatches it to the `hostRpc.onEvent(type, …)`
//!   registration. No reply is sent, and a plugin with no registration
//!   simply never hears it.
//! - [`CREDIT_CHANNEL_ID`] (2) — stream flow control + cancellation, host →
//!   JS only, JSON frames: grants `{"sid":n,"n":k}` re-credit stream `n`'s
//!   pump, cancels `{"sid":n,"cancel":true}` abort it. Cancels are fired by
//!   [`StreamRx`] drop/`cancel()` and by failed [`RpcClient::open_stream`]s.
//!
//! **Flow control**: the host grants [`CREDIT_WINDOW`] credits up front and
//! one more per chunk a consumer receives ([`StreamRx::recv`] acks
//! ack-on-consumption, TCP-like). At most `CREDIT_WINDOW` data frames are
//! ever in flight — never more than the router's channel capacity, so gated
//! pushes can't be dropped; a slow consumer *stalls* the JS pump (which
//! stops pulling `body.next()`, propagating through `tur:net`'s byte budget
//! down to TCP) instead of losing bytes. Cancellation rides the same
//! channel: dropping the [`StreamRx`] tells the pump to stop, which runs the
//! opener's `release()` hook (conventionally `task.cancel()` — the
//! deterministic wire abort).
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
use tur_engine::core::plugin::{Plugin, PluginRegisterContext};
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

/// Stream credit channel: host → JS grants (`{"sid",n}`) and cancels
/// (`{"sid","cancel"}`) that pace and abort streaming pumps (see
/// [`RpcClient::open_stream`] / [`StreamRx`]).
pub const CREDIT_CHANNEL_ID: u64 = 2;

/// Per-stream credit window: the maximum number of data frames allowed in
/// flight (bus queue + router channel) per open stream. Equals the router's
/// mpsc capacity, so a credit-gated push can never find the channel full —
/// a Full hit indicates a protocol violation and is dropped with a warning.
pub const CREDIT_WINDOW: u32 = 32;

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

/// Which side a control-RPC request is issued on. Every envelope carries it
/// explicitly, and the JS dispatcher routes strictly by it — an op registered
/// in the wrong namespace is a "no host/view handler" error at the first
/// call, never a silent half-serve.
///
/// - [`RpcScope::Host`] — the Rust host invoking the backend: the storage
///   contract (`storage:list` / `storage:get` via `ease-js-storage`), the
///   instance lifecycle (`storage:removeInstance`), and the OAuth flow
///   (`oauth:url` / `oauth:exchange`) — all contract literals with identity
///   riding the payload. Resolves `hostRpc.registerHandler` /
///   `hostRpc.registerStream` registrations.
/// - [`RpcScope::View`] — the plugin's own view invoking its backend via
///   `ease.rpc.call`. Resolves `viewRpc.registerHandler` registrations (views
///   cannot open streams).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RpcScope {
    Host,
    View,
}

#[derive(Serialize)]
struct OutgoingRequest<'a> {
    id: u64,
    op: &'a str,
    scope: RpcScope,
    /// Envelope-level stream id for `hostRpc.registerStream` opens (host
    /// only). The `args` object is passed through verbatim — no magic-key
    /// injection; plugin code never sees a stream id.
    #[serde(rename = "streamId", skip_serializing_if = "Option::is_none")]
    stream_id: Option<u32>,
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
/// JS module (dispatcher + `registerStream` pump + credit gates). Then call
/// [`RpcClient::wire`] on each instance to obtain a caller handle.
pub struct TurRpcPlugin;

impl Plugin for TurRpcPlugin {
    fn register(&self, ctx: &mut PluginRegisterContext<'_>) -> Result<(), TurError> {
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

    /// Invoke a **host-scoped** JS handler — one registered via
    /// `hostRpc.registerHandler` — and await its result. For the Rust host's
    /// backend calls: the storage contract and the bridged lifecycle ops.
    /// Errors thrown by the handler come back as [`RpcError::Handler`].
    pub async fn call_host(&self, op: &str, args: Value) -> Result<Value, RpcError> {
        self.call_inner(RpcScope::Host, op, args, None).await
    }

    /// Invoke a **view-scoped** JS handler — one registered via
    /// `viewRpc.registerHandler`. This is the scope `ease.rpc.call` from a
    /// plugin view lands in (see `rpc_bridge.rs`); Rust code rarely calls it
    /// directly.
    pub async fn call_view(&self, op: &str, args: Value) -> Result<Value, RpcError> {
        self.call_inner(RpcScope::View, op, args, None).await
    }

    async fn call_inner(
        &self,
        scope: RpcScope,
        op: &str,
        args: Value,
        stream_id: Option<u32>,
    ) -> Result<Value, RpcError> {
        let id = self.next_req_id.fetch_add(1, Ordering::Relaxed);
        let (otx, orx) = oneshot::channel();
        // Register the awaiter BEFORE emitting so a fast (impossible in
        // practice, but defensive) reply is never missed.
        self.pending.lock().unwrap().insert(id, otx);
        let payload = serde_json::to_vec(&OutgoingRequest {
            id,
            op,
            scope,
            stream_id,
            args: &args,
        })
        .unwrap_or_default();
        // Self-waking: emit_to_js enqueues a WorkerMsg → worker flushes → JS
        // dispatcher runs → reply ships back via MainMsg::EventBusToHost →
        // router resolves the oneshot.
        self.eb.emit_to_js(RPC_CHANNEL_ID, payload);
        orx.await.map_err(|_| RpcError::ChannelClosed)?
    }

    /// Fire a plugin event at the JS realm on the dedicated event channel:
    /// pushes `{type, payload}` JSON, delivered to the
    /// `hostRpc.onEvent(type, …)` registration on the next flush.
    /// Fire-and-forget — no reply is sent, and a plugin with no registration
    /// silently never hears it (standard channel semantics since tur #190).
    pub fn emit_event(&self, event_type: &str, payload: Value) {
        let frame = serde_json::json!({ "type": event_type, "payload": payload });
        let bytes = serde_json::to_vec(&frame).unwrap_or_default();
        self.eb.emit_to_js(EVENT_CHANNEL_ID, bytes);
    }

    /// Open a byte stream: calls JS stream opener `op` (registered via
    /// `hostRpc.registerStream` — streams are host-only; views cannot open
    /// them) and awaits its metadata reply, returning the
    /// metadata plus a [`StreamRx`] for the chunks the JS pump pushes. The
    /// stream id is host-assigned and carried in the request *envelope*
    /// (`streamId` alongside `op`/`args`) — the opener receives `args`
    /// verbatim, and plugin code never sees a stream id.
    ///
    /// Flow control: [`CREDIT_WINDOW`] credits are granted up front; each
    /// [`StreamRx::recv`] of a data chunk re-credits the pump by one, so the
    /// consumer's pace gates the producer's. Dropping the [`StreamRx`] (or
    /// [`StreamRx::cancel`]) aborts the pump.
    pub async fn open_stream(
        &self,
        op: &str,
        args: Value,
    ) -> Result<(Value, StreamRx), RpcError> {
        let stream_id = self.next_stream_id.fetch_add(1, Ordering::Relaxed) as u32;
        let (stx, srx) = mpsc::channel::<StreamChunk>(CREDIT_WINDOW as usize);
        self.streams.lock().unwrap().insert(stream_id, stx);
        // Initial credit window — emitted before the request so the credits
        // are already queued when the JS pump starts taking them.
        emit_credit(&self.eb, stream_id, CREDIT_WINDOW);
        // Drop guard: ANY failure exit — the call erroring, or the caller
        // dropping this future mid-await — removes the table entry (no leak)
        // and cancels the JS pump (the opener may still resolve and start
        // pumping on the initial credits; the cancel is what unparks it).
        let guard = StreamGuard {
            streams: self.streams.clone(),
            eb: self.eb.clone(),
            sid: stream_id,
            armed: true,
        };
        match self.call_inner(RpcScope::Host, op, args, Some(stream_id)).await {
            Ok(metadata) => {
                guard.disarm();
                Ok((
                    metadata,
                    StreamRx {
                        rx: srx,
                        eb: self.eb.clone(),
                        sid: stream_id,
                        done: false,
                    },
                ))
            }
            // guard fires on drop: entry removed + cancel emitted
            Err(e) => Err(e),
        }
    }
}

/// Cleans up a failed/abandoned `open_stream`: removes the stream-table
/// entry and emits a cancel frame, unless defused by a successful open.
struct StreamGuard {
    streams: StreamTable,
    eb: EventBusHandle,
    sid: u32,
    armed: bool,
}

impl StreamGuard {
    fn disarm(mut self) {
        self.armed = false;
    }
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        if self.armed {
            self.streams.lock().unwrap().remove(&self.sid);
            emit_cancel(&self.eb, self.sid);
        }
    }
}

// ---------------------------------------------------------------------------
// StreamRx — the consumer-side handle (credits + cancellation)
// ---------------------------------------------------------------------------

/// Receiving half of an open stream. Every `recv()` of a [`StreamChunk::Data`]
/// re-credits the JS pump by one chunk (the backpressure ack); dropping the
/// handle — or [`StreamRx::cancel`] — emits a cancel frame that aborts the
/// pump (its `release` hook then runs, conventionally `task.cancel()` on the
/// underlying request).
pub struct StreamRx {
    rx: mpsc::Receiver<StreamChunk>,
    eb: EventBusHandle,
    sid: u32,
    done: bool,
}

impl StreamRx {
    /// Await the next stream item. `None` means the stream finished (an
    /// `End`/`Error` was already delivered, or the sender vanished).
    pub async fn recv(&mut self) -> Option<StreamChunk> {
        let chunk = self.rx.recv().await?;
        match chunk {
            StreamChunk::Data(_) => {
                // Ack-on-consumption: one grant back per chunk handed out.
                emit_credit(&self.eb, self.sid, 1);
            }
            StreamChunk::End | StreamChunk::Error(_) => {
                self.done = true;
            }
        }
        Some(chunk)
    }

    /// Abort the stream: the JS pump stops pushing and its `release` hook
    /// fires. Idempotent; also runs automatically on drop.
    pub fn cancel(&mut self) {
        if !self.done {
            self.done = true;
            emit_cancel(&self.eb, self.sid);
        }
    }
}

impl Drop for StreamRx {
    fn drop(&mut self) {
        self.cancel();
    }
}

fn emit_credit(eb: &EventBusHandle, sid: u32, n: u32) {
    let frame = serde_json::json!({ "sid": sid, "n": n });
    if let Ok(bytes) = serde_json::to_vec(&frame) {
        eb.emit_to_js(CREDIT_CHANNEL_ID, bytes);
    }
}

fn emit_cancel(eb: &EventBusHandle, sid: u32) {
    let frame = serde_json::json!({ "sid": sid, "cancel": true });
    if let Ok(bytes) = serde_json::to_vec(&frame) {
        eb.emit_to_js(CREDIT_CHANNEL_ID, bytes);
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
        match sender.try_send(chunk) {
            Ok(()) => {}
            // Unreachable for well-formed pumps — the credit window equals
            // the channel capacity (see CREDIT_WINDOW); warn loudly if a
            // protocol violation ever lands here.
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!("tur:rpc: stream {sid} chunk dropped (channel full)");
            }
            // Consumer gone; its StreamRx drop already emitted a cancel.
            Err(mpsc::error::TrySendError::Closed(_)) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// tur:rpc JS glue — scoped dispatch (hostRpc/viewRpc), credit-gated
// StreamProducer pump, plugin events. The module source lives in
// `tur-rpc.mjs` next to this file (registered under the specifier
// "tur:rpc"); the bus channel ids there must match the Rust constants
// RPC_CHANNEL_ID / EVENT_CHANNEL_ID / CREDIT_CHANNEL_ID above.
// ---------------------------------------------------------------------------

const RPC_JS: &str = include_str!("tur-rpc.mjs");
