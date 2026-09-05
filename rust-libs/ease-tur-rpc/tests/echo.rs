//! Round-trip tests for `ease-tur-rpc`: the host (on a tokio thread) calls a
//! JS handler (sync / async / throwing) and awaits its result, mediated by the
//! tur event bus + id correlation. Exercises the production path end-to-end
//! (EventBusHandle.emit_to_js → worker flush → JS dispatcher → reply →
//! MainMsg::EventBusToHost → on_bus_event router → oneshot), just with the
//! test harness (`TurTestApp`) driving frames instead of an Android FrameLoop.
//!
//! The streaming section covers the `hostRpc.registerStream` path: metadata
//! reply, credit-gated pumping (including a slow-consumer burst that would
//! lose chunks ungated), `release`-exactly-once on every exit, `mapError`,
//! and host-side cancellation via `StreamRx` drop. The scope-isolation
//! section pins the `hostRpc`/`viewRpc` namespace split.

use std::sync::{Arc, Mutex};

use ease_tur_rpc::{RpcClient, RpcError, RpcScope, StreamChunk, TurRpcPlugin};
use tur_engine::core::plugin::Plugin;
use tur_integration_tests::TurTestApp;

const RPC_JS: &str = r#"
import { hostRpc, viewRpc } from "tur:rpc";

// sync handler — returns args unchanged
hostRpc.registerHandler("echo", (args) => args);

// async handler — awaits a microtask then returns args
hostRpc.registerHandler("asyncEcho", async (args) => {
  await Promise.resolve();
  return args;
});

// throwing handler
hostRpc.registerHandler("fail", () => { throw new Error("boom"); });

// view-scoped twin of echo, for the scope-isolation tests
viewRpc.registerHandler("viewEcho", (args) => args);
"#;

fn build_app() -> TurTestApp {
    let extra: Vec<Box<dyn Plugin>> = vec![Box::new(TurRpcPlugin)];
    TurTestApp::new_with_extra_plugins(200.0, 100.0, extra).expect("test app")
}

/// Run `client.call_<scope>(op, args)` on a dedicated tokio runtime (worker
/// thread) while the test thread pumps frames via `wait_for`. Mirrors
/// production: caller on tokio, JS on the worker thread.
fn call_with_pump(
    app: &mut TurTestApp,
    client: RpcClient,
    scope: RpcScope,
    op: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let slot: Arc<Mutex<Option<Result<serde_json::Value, RpcError>>>> = Arc::new(Mutex::new(None));
    let slot_for_task = slot.clone();
    let op = op.to_string();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.spawn(async move {
        let r = match scope {
            RpcScope::Host => client.call_host(&op, args).await,
            RpcScope::View => client.call_view(&op, args).await,
        };
        *slot_for_task.lock().unwrap() = Some(r);
    });

    app.wait_for(|_| slot.lock().unwrap().is_some());
    let result = slot.lock().unwrap().take().expect("result settled");
    drop(rt);
    result
}

#[test]
fn sync_handler_round_trip() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(RPC_JS).expect("load js");

    let result = call_with_pump(&mut app, client, RpcScope::Host, "echo", serde_json::json!({ "value": 42 }))
        .expect("call ok");
    assert_eq!(result, serde_json::json!({ "value": 42 }));
}

#[test]
fn async_handler_round_trip() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(RPC_JS).expect("load js");

    let result = call_with_pump(
        &mut app,
        client,
        RpcScope::Host,
        "asyncEcho",
        serde_json::json!([1, "two", { "three": 3 }]),
    )
    .expect("call ok");
    assert_eq!(result, serde_json::json!([1, "two", { "three": 3 }]));
}

#[test]
fn handler_error_propagates() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(RPC_JS).expect("load js");

    let err = call_with_pump(&mut app, client, RpcScope::Host, "fail", serde_json::json!(null))
        .expect_err("handler should have thrown");
    match err {
        RpcError::Handler(msg) => assert!(msg.contains("boom"), "got: {msg}"),
        other => panic!("expected Handler error, got {other:?}"),
    }
}

#[test]
fn unknown_op_returns_error() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(RPC_JS).expect("load js");

    let err = call_with_pump(&mut app, client, RpcScope::Host, "nope", serde_json::json!(null))
        .expect_err("unknown op should error");
    match err {
        RpcError::Handler(msg) => assert!(msg.contains("no host handler"), "got: {msg}"),
        other => panic!("expected Handler error, got {other:?}"),
    }
}

// === Streaming (registerStream, credit-gated) ==============================

const STREAM_JS: &str = r#"
import { hostRpc } from "tur:rpc";
import { encodeUtf8 } from "tur:std";

// probe state
const state = { released: 0, pulled: 0 };
hostRpc.registerHandler("state", () => ({ released: state.released, pulled: state.pulled }));

// plain-object body yielding one string per next() — no generators, the
// tur:net body shape (next + Symbol.asyncIterator).
function bodyOf(parts, failAt) {
  let i = 0;
  return {
    [Symbol.asyncIterator]() { return this; },
    next() {
      state.pulled += 1;
      if (i < parts.length) {
        const v = parts[i];
        i += 1;
        if (failAt !== undefined && i === failAt) {
          return Promise.reject(new Error(v));
        }
        return Promise.resolve({ done: false, value: encodeUtf8(v) });
      }
      return Promise.resolve({ done: true });
    },
  };
}

// pushes three chunks then ends — concatenates to "Hello!"
hostRpc.registerStream("blob", () => ({
  meta: { size: 6 },
  body: bodyOf(["Hel", "lo", "!"]),
  release: () => { state.released += 1; },
}));

// opener rejects — the RPC itself must fail (single error path)
hostRpc.registerStream("failing", () => Promise.reject(new Error("boom")));

// body fails mid-way; mapError marks the error before it reaches the host
hostRpc.registerStream("marked", () => ({
  meta: {},
  body: bodyOf(["a", "inner"], 2),
  mapError: (e) => new Error("MARKED: " + (e && e.message)),
  release: () => { state.released += 1; },
}));

// resolves without a body — protocol violation, rejected with a clear message
hostRpc.registerStream("badshape", () => Promise.resolve({ meta: {} }));

// burst: 48 one-char chunks — the slow-consumer credit test
hostRpc.registerStream("burst", () => {
  const parts = [];
  // 48 chunks > the 32-credit window, so the pump must ride re-grants
  for (let i = 0; i < 48; i++) parts.push(String(i % 10));
  return { meta: { size: 48 }, body: bodyOf(parts) };
});

// long body for the cancel test (more chunks than the credit window)
hostRpc.registerStream("long", () => {
  const parts = [];
  for (let i = 0; i < 100; i++) parts.push(String(i % 10));
  return { meta: {}, body: bodyOf(parts), release: () => { state.released += 1; } };
});

// args must arrive VERBATIM — the stream id rides the envelope, never a
// magic key inside args
let lastArgs = null;
hostRpc.registerStream("argcheck", (args) => {
  lastArgs = args;
  return { meta: {}, body: bodyOf([]) };
});
hostRpc.registerHandler("lastArgs", () => lastArgs);
"#;

/// Open a stream on a tokio worker, collect chunks until End/Error, pumping
/// frames on the test thread meanwhile.
fn stream_with_pump(
    app: &mut TurTestApp,
    client: RpcClient,
    op: &str,
    args: serde_json::Value,
) -> (serde_json::Value, Result<Vec<u8>, String>) {
    let slot: Arc<Mutex<Option<(serde_json::Value, Result<Vec<u8>, String>)>>> =
        Arc::new(Mutex::new(None));
    let slot_for_task = slot.clone();
    let op = op.to_string();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.spawn(async move {
        let (meta, mut rx) = match client.open_stream(&op, args).await {
            Ok(v) => v,
            Err(e) => {
                *slot_for_task.lock().unwrap() = Some((serde_json::Value::Null, Err(format!("{e:?}"))));
                return;
            }
        };
        let mut buf = Vec::new();
        let outcome = loop {
            match rx.recv().await {
                None => break Ok(buf),
                Some(StreamChunk::Data(b)) => buf.extend_from_slice(&b),
                Some(StreamChunk::End) => break Ok(buf),
                Some(StreamChunk::Error(m)) => break Err(m),
            }
        };
        *slot_for_task.lock().unwrap() = Some((meta, outcome));
    });

    app.wait_for(|_| slot.lock().unwrap().is_some());
    let result = slot.lock().unwrap().take().expect("stream settled");
    drop(rt);
    result
}

#[test]
fn open_stream_delivers_chunks_until_end() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STREAM_JS).expect("load js");

    let (meta, outcome) = stream_with_pump(&mut app, client.clone(), "blob", serde_json::json!({}));
    let buf = outcome.expect("stream should complete cleanly");
    assert_eq!(meta, serde_json::json!({ "size": 6 }));
    assert_eq!(&buf, b"Hello!");

    // release fired exactly once on the normal exit path
    let mut released = 0;
    for _ in 0..50 {
        let v = call_with_pump(&mut app, client.clone(), RpcScope::Host, "state", serde_json::json!(null))
            .expect("probe call ok");
        if v.get("released").and_then(|x| x.as_i64()) == Some(1) {
            released = 1;
            break;
        }
    }
    assert_eq!(released, 1, "release should fire exactly once after normal end");
}

#[test]
fn open_stream_opener_failure_fails_the_call() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STREAM_JS).expect("load js");

    let (meta, outcome) = stream_with_pump(&mut app, client, "failing", serde_json::json!({}));
    assert!(meta.is_null(), "no metadata on opener failure");
    let err = outcome.expect_err("opener failure should fail open_stream");
    assert!(err.contains("boom"), "got: {err}");
}

#[test]
fn open_stream_bad_shape_rejected() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STREAM_JS).expect("load js");

    let (_meta, outcome) = stream_with_pump(&mut app, client, "badshape", serde_json::json!({}));
    let err = outcome.expect_err("missing body should fail open_stream");
    assert!(err.contains("must resolve"), "got: {err}");
}

#[test]
fn midbody_error_applies_map_error() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STREAM_JS).expect("load js");

    let (_meta, outcome) = stream_with_pump(&mut app, client, "marked", serde_json::json!({}));
    let err = outcome.expect_err("body failure should error the stream");
    assert!(err.contains("MARKED: inner"), "got: {err}");
}

/// The stream id rides the request envelope — the opener's args must arrive
/// verbatim, with no injected `streamId` key.
#[test]
fn stream_opener_receives_args_verbatim() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STREAM_JS).expect("load js");

    let (_meta, outcome) =
        stream_with_pump(&mut app, client.clone(), "argcheck", serde_json::json!({ "x": 1 }));
    outcome.expect("stream should complete cleanly");

    let mut settled = None;
    for _ in 0..50 {
        let v = call_with_pump(&mut app, client.clone(), RpcScope::Host, "lastArgs", serde_json::json!(null))
            .expect("probe call ok");
        if !v.is_null() {
            settled = Some(v);
            break;
        }
    }
    let args = settled.expect("opener should have observed its args");
    assert_eq!(args, serde_json::json!({ "x": 1 }), "args must be verbatim");
}

/// Burst 300 chunks past the credit window while the consumer sleeps between
/// recvs — ungated pushes would overflow the router channel and drop chunks;
/// the credit gate must deliver every byte intact.
#[test]
fn credit_gate_keeps_slow_consumer_lossless() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STREAM_JS).expect("load js");

    let client_for_task = client.clone();
    let slot: Arc<Mutex<Option<Result<Vec<u8>, String>>>> = Arc::new(Mutex::new(None));
    let slot_for_task = slot.clone();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.spawn(async move {
        let (meta, mut rx) = client_for_task
            .open_stream("burst", serde_json::json!({}))
            .await
            .expect("open");
        assert_eq!(meta.get("size").and_then(|s| s.as_i64()), Some(48));
        let mut buf = Vec::new();
        let outcome = loop {
            // slow consumer: 1ms pause between chunks
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
            match rx.recv().await {
                None => break Ok(buf),
                Some(StreamChunk::Data(b)) => buf.extend_from_slice(&b),
                Some(StreamChunk::End) => break Ok(buf),
                Some(StreamChunk::Error(m)) => break Err(m),
            }
        };
        *slot_for_task.lock().unwrap() = Some(outcome);
    });

    // Each wait_for call advances at most 2000 virtual ms; every chunk past
    // the credit window costs a real-time grant round-trip, so loop the wait.
    let mut settled = false;
    for _ in 0..30 {
        if app.wait_for(|_| slot.lock().unwrap().is_some()) {
            settled = true;
            break;
        }
    }
    assert!(settled, "burst stream should settle within the extended budget");
    let outcome = slot.lock().unwrap().take().expect("stream settled");
    drop(rt);
    let buf = outcome.expect("burst should complete cleanly");
    let expected: Vec<u8> = (0..48).map(|i| b'0' + (i % 10) as u8).collect();
    assert_eq!(buf.len(), 48, "no chunk may be dropped under backpressure");
    assert_eq!(buf, expected);
}

/// Dropping the `StreamRx` mid-stream cancels the JS pump: it stops pushing
/// and runs the opener's `release` hook exactly once.
#[test]
fn cancel_on_stream_rx_drop_releases_pump() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STREAM_JS).expect("load js");

    let client_for_task = client.clone();
    let slot: Arc<Mutex<Option<u32>>> = Arc::new(Mutex::new(None));
    let slot_for_task = slot.clone();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.spawn(async move {
        let (_meta, mut rx) = client_for_task
            .open_stream("long", serde_json::json!({}))
            .await
            .expect("open long stream");
        // consume a few chunks, then abandon the stream
        let mut got = 0u32;
        while let Some(chunk) = rx.recv().await {
            match chunk {
                StreamChunk::Data(_) => {
                    got += 1;
                    if got >= 5 {
                        break;
                    }
                }
                _ => break,
            }
        }
        drop(rx); // → cancel frame → JS pump aborts → release()
        *slot_for_task.lock().unwrap() = Some(got);
    });

    app.wait_for(|_| slot.lock().unwrap().is_some());
    let got = slot.lock().unwrap().take().expect("consumer settled");
    drop(rt);
    assert!(got >= 1, "should have received some chunks before cancelling");

    // the pump's release hook fires (exactly once) after the cancel
    let mut released = 0;
    for _ in 0..100 {
        let v = call_with_pump(&mut app, client.clone(), RpcScope::Host, "state", serde_json::json!(null))
            .expect("probe call ok");
        if v.get("released").and_then(|x| x.as_i64()) == Some(1) {
            released = 1;
            break;
        }
    }
    assert_eq!(released, 1, "cancel should run release exactly once");
}

// === Scope isolation (hostRpc vs viewRpc) ==================================

const SCOPE_JS: &str = r#"
import { hostRpc, viewRpc } from "tur:rpc";

hostRpc.registerHandler("hostOnly", (args) => ({ from: "host", args }));
viewRpc.registerHandler("viewOnly", (args) => ({ from: "view", args }));
"#;

#[test]
fn view_scoped_call_reaches_view_handler() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(SCOPE_JS).expect("load js");

    let result = call_with_pump(
        &mut app,
        client,
        RpcScope::View,
        "viewOnly",
        serde_json::json!({ "x": 1 }),
    )
    .expect("view-scoped call should reach the viewRpc registration");
    assert_eq!(
        result,
        serde_json::json!({ "from": "view", "args": { "x": 1 } })
    );
}

#[test]
fn host_call_cannot_reach_view_op() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(SCOPE_JS).expect("load js");

    let err = call_with_pump(&mut app, client, RpcScope::Host, "viewOnly", serde_json::json!({}))
        .expect_err("host scope must not serve a viewRpc op");
    match err {
        RpcError::Handler(msg) => assert!(msg.contains("no host handler"), "got: {msg}"),
        other => panic!("expected Handler error, got {other:?}"),
    }
}

#[test]
fn view_call_cannot_reach_host_op() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(SCOPE_JS).expect("load js");

    let err = call_with_pump(&mut app, client, RpcScope::View, "hostOnly", serde_json::json!({}))
        .expect_err("view scope must not serve a hostRpc op");
    match err {
        RpcError::Handler(msg) => assert!(msg.contains("no view handler"), "got: {msg}"),
        other => panic!("expected Handler error, got {other:?}"),
    }
}

// === Plugin events (fire-and-forget channel 1) ==============================

const EVENT_JS: &str = r#"
import { hostRpc } from "tur:rpc";

let got = null;
hostRpc.onEvent("ping", (payload) => { got = payload.value; });
hostRpc.registerHandler("got", () => ({ value: got }));
"#;

#[test]
fn emit_event_reaches_on_event_registration() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(EVENT_JS).expect("load js");

    // Fire-and-forget on the event channel; observe via an RPC round-trip.
    // Delivery is asynchronous (worker flush), so poll until it lands.
    client.emit_event("ping", serde_json::json!({ "value": 7 }));
    let mut delivered = false;
    for _ in 0..50 {
        let v = call_with_pump(&mut app, client.clone(), RpcScope::Host, "got", serde_json::json!(null))
            .expect("probe call ok");
        if v.get("value").and_then(|x| x.as_i64()) == Some(7) {
            delivered = true;
            break;
        }
    }
    assert!(delivered, "event should have been delivered to onEvent");
}
