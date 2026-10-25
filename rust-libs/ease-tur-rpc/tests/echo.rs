//! End-to-end host⇄rut RPC tests: a real rut fixture backend registers ops
//! + events through the `ease.rpc` rows ([`TurRpcBusPlugin`]) and answers
//! dispatches driven through [`RpcClient`].
//!
//! The harness app (`!Send`, Rc internals) lives on the driver thread and
//! never escapes it — the test drives everything through the client, whose
//! [`RutDriver`] hops commands onto that thread over a channel. This is the
//! same shape as production (the Android driver hops onto the tur-host
//! thread); there is no unsafe anywhere.
//!
//! Covers: the stash-and-take dispatch (one entry fn per op), the reply
//! envelope (`ok`/`error`), unknown ops, the context-crossing token
//! (downcast inside the fixture), event fan-out + unsubscribed no-ops, and
//! the not-booted transport error.

use std::sync::mpsc;
use std::sync::Arc;

use ease_tur_rpc::{RpcBus, RpcClient, RpcError, RutDriver, TurRpcBusPlugin};
use serde_json::json;
use tur_integration_tests::TurTestApp;

/// The fixture backend: one entry fn per contract op + one event handler,
/// registered at `start` through the bus rows. `op_echo` answers the whole
/// request JSON as the result (proves the stash-and-take round-trip);
/// `op_greet` downcasts the context and reads its field (proves the
/// crossing); `op_fail` answers the error envelope.
const ECHO_RUT: &str = r#"
use tur_host::ease::rpc::{ register, subscribe_event, take_request };

// The context the embedder holds between entry calls.
struct Ctx {
    greeting: str,
}

entry fn start() -> opaque {
    register("echo", "op_echo");
    register("greet", "op_greet");
    register("fail", "op_fail");
    subscribe_event("music:play", "on_play");
    return opaque(Ctx { greeting: "hello" });
}

// Echo the entire request JSON back as the result string — the host
// asserts the exact stash-and-take round-trip.
entry fn op_echo(cx: opaque) -> str {
    let _ = opaque::downcast<Ctx>(cx);
    let req = take_request();
    return f"{{\"ok\":true,\"result\":{req}}}";
}

// Read the context record — proves the crossing + the downcast.
entry fn op_greet(cx: opaque) -> str {
    let c = opaque::downcast<Ctx>(cx);
    if (c == nil) {
        panic("greet: cx is not a Ctx");
    }
    return f"{{\"ok\":true,\"result\":\"{c.greeting}\"}}";
}

// The plugin's own failure — the error envelope surfaces as
// RpcError::Handler on the client.
entry fn op_fail(cx: opaque) -> str {
    let _ = opaque::downcast<Ctx>(cx);
    return "{\"ok\":false,\"error\":\"boom\"}";
}

// The event handler: drain the stash, answer ok (the answer is ignored —
// the host's event dispatch is fire-and-forget).
entry fn on_play(cx: opaque) -> str {
    let _ = opaque::downcast<Ctx>(cx);
    let req = take_request();
    return f"{{\"ok\":true,\"result\":{req}}}";
}
"#;

/// The driver's command rail (all payloads `Send` — the app never is).
enum Cmd {
    CallStr(String, u64, mpsc::SyncSender<Result<String, String>>),
    Call(String, u64, mpsc::SyncSender<Result<(), String>>),
    Token(mpsc::SyncSender<u64>),
}

impl RutDriver for TestDriver {
    fn call_entry_str(&self, name: &str, cx: u64) -> Result<String, String> {
        let (rtx, rrx) = mpsc::sync_channel(1);
        self.tx
            .send(Cmd::CallStr(name.to_string(), cx, rtx))
            .map_err(|_| "driver thread gone".to_string())?;
        rrx.recv().map_err(|_| "driver dropped".to_string())?
    }

    fn call_entry(&self, name: &str, cx: u64) -> Result<(), String> {
        let (rtx, rrx) = mpsc::sync_channel(1);
        self.tx
            .send(Cmd::Call(name.to_string(), cx, rtx))
            .map_err(|_| "driver thread gone".to_string())?;
        rrx.recv()
            .map_err(|_| "driver dropped".to_string())?
    }

    fn start_token(&self) -> u64 {
        let (rtx, rrx) = mpsc::sync_channel(1);
        self.tx.send(Cmd::Token(rtx)).ok();
        rrx.recv().unwrap_or(0)
    }
}

struct TestDriver {
    tx: mpsc::Sender<Cmd>,
}

/// Boot the fixture: the harness app is constructed on (and stays on) the
/// driver thread; the test gets only the client over the shared bus.
fn boot() -> RpcClient {
    let bus = RpcBus::new();
    let (tx, rx) = mpsc::channel::<Cmd>();
    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let bus_for_plugin = Some(bus.clone());
    std::thread::spawn(move || {
        let app = TurTestApp::new_with_extra_plugins(
            400.0,
            400.0,
            vec![Box::new(TurRpcBusPlugin::new(bus_for_plugin))],
        )
        .expect("harness app");
        app.load_rut_module(ECHO_RUT).expect("fixture loads");
        ready_tx.send(()).expect("test still waiting");
        for cmd in rx {
            match cmd {
                Cmd::CallStr(name, cx, rtx) => {
                    let _ = rtx.send(
                        app.call_rut_entry_cx_str(&name, cx).map_err(|e| e.to_string()),
                    );
                }
                Cmd::Call(name, cx, rtx) => {
                    let _ = rtx.send(
                        app.call_rut_entry_cx(&name, cx)
                            .map(|_| ())
                            .map_err(|e| e.to_string()),
                    );
                }
                Cmd::Token(rtx) => {
                    let _ = rtx.send(app.rut_start_answer());
                }
            }
        }
    });
    ready_rx.recv().expect("driver thread booted");
    RpcClient::new(bus, Arc::new(TestDriver { tx }))
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn dispatch_roundtrips_the_request_through_the_stash() {
    let client = boot();
    let reply = runtime()
        .block_on(client.call_host("echo", json!({ "msg": "hi" })))
        .expect("echo ok");
    assert_eq!(
        reply,
        json!({ "op": "echo", "args": { "msg": "hi" } }),
        "the fixture echoes the whole request envelope as the result"
    );
}

#[test]
fn the_context_crosses_and_downcasts() {
    let client = boot();
    let reply = runtime()
        .block_on(client.call_host("greet", json!({})))
        .expect("greet ok");
    assert_eq!(reply, json!("hello"), "the fixture read the ctx record's field");
}

#[test]
fn error_envelopes_surface_as_handler_errors() {
    let client = boot();
    let err = runtime()
        .block_on(client.call_host("fail", json!({})))
        .expect_err("fail answers an error envelope");
    match err {
        RpcError::Handler(msg) => assert_eq!(msg, "boom"),
        other => panic!("expected Handler, got {other:?}"),
    }
}

#[test]
fn unknown_ops_are_rejected_client_side() {
    let client = boot();
    let err = runtime()
        .block_on(client.call_host("nope", json!({})))
        .expect_err("unregistered op");
    assert!(matches!(err, RpcError::UnknownOp(op) if op == "nope"));
}

#[test]
fn events_fan_out_and_unsubscribed_types_noop() {
    let client = boot();
    // Subscribed: dispatched (fire-and-forget — no result to assert; a
    // failed dispatch would log, not panic).
    runtime().block_on(client.emit_event("music:play", json!({ "musicId": "7" })));
    // Unsubscribed: a silent no-op.
    runtime().block_on(client.emit_event("music:stop", json!({})));
}

#[test]
fn a_not_booted_module_is_a_transport_error() {
    // A client over a bus whose driver reports no start token (the module
    // failed to boot / was never loaded).
    struct DeadDriver;
    impl RutDriver for DeadDriver {
        fn call_entry_str(&self, _name: &str, _cx: u64) -> Result<String, String> {
            Err("dead".into())
        }
        fn call_entry(&self, _name: &str, _cx: u64) -> Result<(), String> {
            Err("dead".into())
        }
        fn start_token(&self) -> u64 {
            0
        }
    }
    // The op IS registered (the module's start ran far enough to bind it),
    // but the boot never answered a context token.
    let bus = RpcBus::new();
    bus.register_op("echo", "op_echo");
    let client = RpcClient::new(bus, Arc::new(DeadDriver));
    let err = runtime()
        .block_on(client.call_host("echo", json!({})))
        .expect_err("not booted");
    assert!(matches!(err, RpcError::Transport(_)));
}
