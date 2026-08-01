//! Round-trip spike for `ease-tur-rpc`: proves the host can call a JS handler
//! (sync and async) and await its result across threads, and that handler
//! errors propagate. This is the de-risk gate for the JS storage-provider
//! (OneDrive) work — it exercises the exact production path: a caller on a
//! tokio thread awaits a result computed in the JS realm on the instance
//! thread, mediated by the event bus + id-correlation.
//!
//! Tests are plain `#[test]` (NOT `#[tokio::test]`): tur's executor panics if
//! `run_frame()` is driven from inside a tokio context ("cannot start a runtime
//! from within a runtime"). So the instance thread (test thread) pumps frames
//! with no tokio, and the caller side runs on a dedicated tokio runtime on a
//! worker thread.

use std::rc::Rc;

use boa_engine::context::time::StdClock;
use ease_tur_rpc::{RpcClient, RpcError, StreamChunk, TurRpcPlugin};
use tur_engine::{TurApp, TurRuntime, TurStdPlugin};
use tur_native::NativeFontLoader;

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

fn build_runtime() -> Rc<TurRuntime> {
    TurRuntime::builder()
        .font_loader(Rc::new(NativeFontLoader::new()))
        .clock(Rc::new(StdClock::new()))
        .plugin(TurStdPlugin)
        .plugin(TurRpcPlugin)
        .build()
        .expect("runtime build")
}

/// Spawn `client.call(...)` on a dedicated tokio runtime (worker thread); pump
/// frames on this thread until it resolves. Mirrors production: caller on
/// tokio, JS on the instance thread.
fn call_with_pump(
    app: &Rc<TurApp>,
    client: RpcClient,
    op: &str,
    args: serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let (done_tx, done_rx) = std::sync::mpsc::channel::<Result<serde_json::Value, RpcError>>();
    let op = op.to_string();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    rt.spawn(async move {
        let _ = done_tx.send(client.call(&op, args).await);
    });

    let mut tries = 0u32;
    let result = loop {
        let _ = app.run_frame();
        match done_rx.try_recv() {
            Ok(r) => break r,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                tries += 1;
                if tries > 50_000 {
                    panic!("rpc round-trip timed out after {tries} frames");
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                panic!("caller task died before sending a result")
            }
        }
    };
    drop(rt);
    result
}

#[test]
fn sync_handler_round_trip() {
    let runtime = build_runtime();
    let app = runtime.create_headless_app((0.0, 0.0)).expect("headless app");
    let client = RpcClient::of(&app).expect("rpc client");
    app.load_module(RPC_JS).expect("load js");

    let result = call_with_pump(&app, client, "echo", serde_json::json!({ "value": 42 }))
        .expect("call ok");
    assert_eq!(result, serde_json::json!({ "value": 42 }));
}

#[test]
fn async_handler_round_trip() {
    let runtime = build_runtime();
    let app = runtime.create_headless_app((0.0, 0.0)).expect("headless app");
    let client = RpcClient::of(&app).expect("rpc client");
    app.load_module(RPC_JS).expect("load js");

    let result = call_with_pump(
        &app,
        client,
        "asyncEcho",
        serde_json::json!([1, "two", { "three": 3 }]),
    )
    .expect("call ok");
    assert_eq!(result, serde_json::json!([1, "two", { "three": 3 }]));
}

#[test]
fn handler_error_propagates() {
    let runtime = build_runtime();
    let app = runtime.create_headless_app((0.0, 0.0)).expect("headless app");
    let client = RpcClient::of(&app).expect("rpc client");
    app.load_module(RPC_JS).expect("load js");

    let err = call_with_pump(&app, client, "fail", serde_json::json!(null))
        .expect_err("handler should have thrown");
    match err {
        RpcError::Handler(msg) => assert!(msg.contains("boom"), "got: {msg}"),
        other => panic!("expected Handler error, got {other:?}"),
    }
}

#[test]
fn unknown_op_returns_error() {
    let runtime = build_runtime();
    let app = runtime.create_headless_app((0.0, 0.0)).expect("headless app");
    let client = RpcClient::of(&app).expect("rpc client");
    app.load_module(RPC_JS).expect("load js");

    let err = call_with_pump(&app, client, "nope", serde_json::json!(null))
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

/// Open a stream on a tokio worker, collect chunks until End/Error on this
/// thread while pumping frames.
fn stream_with_pump(
    app: &Rc<TurApp>,
    client: RpcClient,
    op: &str,
    args: serde_json::Value,
) -> (serde_json::Value, Result<Vec<u8>, String>) {
    let (done_tx, done_rx) = std::sync::mpsc::channel();
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
                let _ = done_tx.send((serde_json::Value::Null, Err(format!("{e:?}"))));
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
        let _ = done_tx.send((meta, outcome));
    });

    let mut tries = 0u32;
    let result = loop {
        let _ = app.run_frame();
        match done_rx.try_recv() {
            Ok(r) => break r,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                tries += 1;
                if tries > 50_000 {
                    panic!("stream round-trip timed out after {tries} frames");
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                panic!("stream task died before sending a result")
            }
        }
    };
    drop(rt);
    result
}

#[test]
fn open_stream_delivers_chunks_until_end() {
    let runtime = build_runtime();
    let app = runtime.create_headless_app((0.0, 0.0)).expect("headless app");
    let client = RpcClient::of(&app).expect("rpc client");
    app.load_module(STREAM_JS).expect("load js");

    let (meta, outcome) = stream_with_pump(&app, client, "blob", serde_json::json!({}));
    let buf = outcome.expect("stream should complete cleanly");
    assert_eq!(meta, serde_json::json!({ "size": 6 }));
    assert_eq!(&buf, b"Hello!");
}

#[test]
fn open_stream_propagates_error() {
    let runtime = build_runtime();
    let app = runtime.create_headless_app((0.0, 0.0)).expect("headless app");
    let client = RpcClient::of(&app).expect("rpc client");
    app.load_module(STREAM_JS).expect("load js");

    let (_meta, outcome) = stream_with_pump(&app, client, "failing", serde_json::json!({}));
    let err = outcome.expect_err("stream should have errored");
    assert!(err.contains("boom"), "got: {err}");
}
