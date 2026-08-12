//! Round-trip tests for `ease-tur-rpc`: the host (on a tokio thread) calls a
//! JS handler (sync / async / throwing) and awaits its result, mediated by the
//! tur event bus + id correlation. Exercises the production path end-to-end
//! (EventBusHandle.emit_to_js → worker flush → JS dispatcher → reply →
//! MainMsg::EventBusToHost → on_bus_event router → oneshot), just with the
//! test harness (`TurTestApp`) driving frames instead of an Android FrameLoop.

use std::sync::{Arc, Mutex};

use ease_tur_rpc::{RpcClient, RpcError, StreamChunk, TurRpcPlugin};
use tur_engine::core::plugin::Plugin;
use tur_integration_tests::TurTestApp;

const RPC_JS: &str = r#"
import { registerHandler } from "tur:rpc";

// sync handler — returns args unchanged
registerHandler("echo", (args) => args);

// async handler — awaits a microtask then returns args
registerHandler("asyncEcho", async (args) => {
  await Promise.resolve();
  return args;
});

// throwing handler
registerHandler("fail", () => { throw new Error("boom"); });
"#;

fn build_app() -> TurTestApp {
    let extra: Vec<Box<dyn Plugin>> = vec![Box::new(TurRpcPlugin)];
    TurTestApp::new_with_extra_plugins(200.0, 100.0, extra).expect("test app")
}

/// Run `client.call(op, args)` on a dedicated tokio runtime (worker thread)
/// while the test thread pumps frames via `wait_for`. Mirrors production:
/// caller on tokio, JS on the worker thread.
fn call_with_pump(
    app: &mut TurTestApp,
    client: RpcClient,
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
        let r = client.call(&op, args).await;
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

    let result = call_with_pump(&mut app, client, "echo", serde_json::json!({ "value": 42 }))
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

    let err = call_with_pump(&mut app, client, "fail", serde_json::json!(null))
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

    let err = call_with_pump(&mut app, client, "nope", serde_json::json!(null))
        .expect_err("unknown op should error");
    match err {
        RpcError::Handler(msg) => assert!(msg.contains("no handler"), "got: {msg}"),
        other => panic!("expected Handler error, got {other:?}"),
    }
}

// === Streaming (byte bridge) ===============================================

const STREAM_JS: &str = r#"
import { registerHandler, pushChunk, endStream, errorStream } from "tur:rpc";
import { encodeUtf8 } from "tur:std";

// pushes three chunks then ends — concatenates to "Hello!"
registerHandler("blob", (args) => {
  const sid = args.streamId;
  pushChunk(sid, encodeUtf8("Hel"));
  pushChunk(sid, encodeUtf8("lo"));
  pushChunk(sid, encodeUtf8("!"));
  endStream(sid);
  return { size: 6 };
});

// immediately errors the stream
registerHandler("failing", (args) => {
  errorStream(args.streamId, "boom");
  return {};
});
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

    let (meta, outcome) = stream_with_pump(&mut app, client, "blob", serde_json::json!({}));
    let buf = outcome.expect("stream should complete cleanly");
    assert_eq!(meta, serde_json::json!({ "size": 6 }));
    assert_eq!(&buf, b"Hello!");
}

#[test]
fn open_stream_propagates_error() {
    let mut app = build_app();
    let client = RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STREAM_JS).expect("load js");

    let (_meta, outcome) = stream_with_pump(&mut app, client, "failing", serde_json::json!({}));
    let err = outcome.expect_err("stream should have errored");
    assert!(err.contains("boom"), "got: {err}");
}
