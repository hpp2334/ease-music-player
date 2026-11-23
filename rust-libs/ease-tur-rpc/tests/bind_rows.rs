//! The bind-table dispatch lane, proven end-to-end on a real rut module.
//!
//! A rut closure (`fn (args: str) -> ?str { … db.greeting … }`) is SEALED
//! as an opaque (`opaque(f)`), filed through the `rpc::bind` row into the
//! host-side per-instance table (keyed by the bind key — the instance's
//! handle stamps the table, the key the slot), and later INVOKED with a
//! raw str (and bytes) through the module's dispatch entry, the result
//! returned. No module-global state anywhere: the closures live host-side,
//! the entry fetches them back by the `cx` handle.
//!
//! The fixture is the plan's shape WITHOUT the `emp` class (rows only) —
//! `emp`'s `Rpc` is a thin forwarder over exactly these rows (the
//! emp-class E2E lives in ease-plugins-check). Driven through
//! [`RpcClient`]'s dispatch lane (the capability probe rides the same
//! entry calls). Phase 5 made this the ONLY contract — there is no other
//! lane left to cover.

use std::sync::Arc;

use ease_tur_rpc::{
    DispatchAnswer, RpcClient, RpcError, RutDriver, TurRpcBindPlugin,
};
use serde_json::json;
use tur_integration_tests::TurTestApp;

/// The fixture backend: the plan's `start(cx)` + `rpc.bind` shape with
/// plain fns, plus the two dispatch entries as two-line forwards over the
/// rows (what `emp`'s `Rpc.dispatch` / `Rpc.dispatch_bin` do). Handlers
/// close over the ctx record (the plan's `db`) — proving the sealed
/// closures carry their captures across the store/invoke gap.
const BIND_RUT: &str = r#"
use rpc_host::rpc::{ bind, handler };

// the state the handlers close over (the plan's `db`)
struct Ctx {
    greeting: str,
    seen: str,
}

// the trap rides a module-level fn, its panic inside an `if` — a tail
// `panic(...)` statement is the missing-return parse repro at this rut
// pin (report material upstream).
fn fail_unauthorized(args: str) -> ?str {
    if (args != "") {
        panic("UNAUTHORIZED: token expired");
    }
    return nil;
}

entry fn start(cx: opaque) {
    let _ = cx;
    // the handlers' shared state — the plan's `db` shape (in a real
    // plugin this is `Db(cx)`, the host-backed KV; the transport spike
    // stands in a local record the closures capture)
    let db = Ctx { greeting: "hello", seen: "" };
    // the plan's shape: one bind per op — the handler decodes its OWN
    // args (raw here; the json lib in real plugins) and answers the raw
    // payload.
    bind(cx, "storage:list", opaque(fn (args: str) -> ?str {
        return f"{{\"listed\":{args},\"by\":\"{db.greeting}\"}}";
    }));
    // the binary lane: raw bytes through, no base64 anywhere.
    bind(cx, "bin:lyric:parse", opaque(fn (args: bytes) -> ?bytes {
        return args;
    }));
    // nil = "not mine" (the lyric dispatch's fall-through arm).
    bind(cx, "bin:lyric:parse:other", opaque(fn (args: bytes) -> ?bytes {
        let _ = args;
        return nil;
    }));
    // an event subscriber: the host dispatches it as `event:<type>` —
    // the handler records the payload into the captured ctx.
    bind(cx, "event:music:play", opaque(fn (args: str) -> ?str {
        db.seen = args;
        return nil;
    }));
    // readback for the event proof (the capture is the same record).
    bind(cx, "test:last-event", opaque(fn (args: str) -> ?str {
        let _ = args;
        return db.seen;
    }));
    // a handler whose panic keeps the prefix law (the host's error arm).
    bind(cx, "fail:unauthorized", opaque(fail_unauthorized));
    // the identity gate: a script-minted handle never binds.
    bind(cx, "test:forge", opaque(fn (args: str) -> ?str {
        let _ = args;
        bind(opaque(987654321), "x", opaque(fn (a: str) -> ?str {
            let _ = a;
            return nil;
        }));
        return "bound";
    }));
    // the probe op's name is RESERVED — this bind must trap (the
    // capability check dispatches it expecting the inert nil arm).
    bind(cx, "test:probe-reserved", opaque(fn (args: str) -> ?str {
        let _ = args;
        bind(opaque(987654321), "__tur_rpc_probe", opaque(fn (a: str) -> ?str {
            let _ = a;
            return "hijacked";
        }));
        return "bound";
    }));
    // the new contract's boot shape: no answer (the identity rode the
    // cx lane, not the return).
}

// the dispatch entry — the two-line forward over the rows.
entry fn rpc(cx: opaque, op: str, args: str) -> ?str {
    let b = handler(cx, op);
    if (b == nil) {
        return nil;
    }
    let f = opaque::downcast<fn(str) -> ?str>(b);
    return f(args);
}

// the binary dispatch entry.
entry fn rpc_bin(cx: opaque, op: str, args: bytes) -> ?bytes {
    let b = handler(cx, f"bin:{op}");
    if (b == nil) {
        return nil;
    }
    let f = opaque::downcast<fn(bytes) -> ?bytes>(b);
    return f(args);
}
"#;

/// The boot identity (the embedder-minted handle id — the value the
/// client dispatches with and the rows decode against).
const BOOT_CX: u64 = 7;

/// The driver's command rail (all payloads `Send` — the app never is).
enum Cmd {
    DispatchStr(
        String,
        u64,
        String,
        String,
        std::sync::mpsc::SyncSender<Result<DispatchAnswer, String>>,
    ),
    DispatchBytes(
        String,
        u64,
        String,
        Vec<u8>,
        std::sync::mpsc::SyncSender<Result<DispatchAnswer, String>>,
    ),
}

impl RutDriver for TestDriver {
    fn call_dispatch_str(
        &self,
        name: &str,
        cx: u64,
        op: &str,
        args: &str,
    ) -> Result<DispatchAnswer, String> {
        let (rtx, rrx) = std::sync::mpsc::sync_channel(1);
        self.tx
            .send(Cmd::DispatchStr(
                name.to_string(),
                cx,
                op.to_string(),
                args.to_string(),
                rtx,
            ))
            .map_err(|_| "driver thread gone".to_string())?;
        rrx.recv().map_err(|_| "driver dropped".to_string())?
    }

    fn call_dispatch_bytes(
        &self,
        name: &str,
        cx: u64,
        op: &str,
        args: &[u8],
    ) -> Result<DispatchAnswer, String> {
        let (rtx, rrx) = std::sync::mpsc::sync_channel(1);
        self.tx
            .send(Cmd::DispatchBytes(
                name.to_string(),
                cx,
                op.to_string(),
                args.to_vec(),
                rtx,
            ))
            .map_err(|_| "driver thread gone".to_string())?;
        rrx.recv().map_err(|_| "driver dropped".to_string())?
    }
}

struct TestDriver {
    tx: std::sync::mpsc::Sender<Cmd>,
}

/// Boot the fixture through the phase-1 cx lane (stamped with the
/// identity) and return the client.
fn boot() -> RpcClient {
    let (tx, rx) = std::sync::mpsc::channel::<Cmd>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let app = TurTestApp::new_with_extra_plugins(
            400.0,
            400.0,
            vec![
                // The standalone bind rows + the fixed identity stamp +
                // the sealed-handler table (the host-test shape of the
                // embedder's stamps).
                Box::new(TurRpcBindPlugin::with_identity(BOOT_CX)),
            ],
        )
        .expect("harness app");
        app.load_rut_module_cx(BIND_RUT, BOOT_CX)
            .expect("fixture loads");
        ready_tx.send(()).expect("test still waiting");
        for cmd in rx {
            match cmd {
                Cmd::DispatchStr(name, cx, op, args, rtx) => {
                    let _ = rtx.send(
                        app.call_rut_entry_cx_str_str(&name, cx, &op, &args)
                            .map(dispatch_answer)
                            .map_err(|e| e.to_string()),
                    );
                }
                Cmd::DispatchBytes(name, cx, op, args, rtx) => {
                    let _ = rtx.send(
                        app.call_rut_entry_cx_str_bytes(&name, cx, &op, &args)
                            .map(dispatch_answer)
                            .map_err(|e| e.to_string()),
                    );
                }
            }
        }
    });
    ready_rx.recv().expect("driver thread booted");
    RpcClient::new(Arc::new(TestDriver { tx }), BOOT_CX)
}

/// The engine's nil-flattened answer map: `Nil` / `Str` / `Bytes` onto the
/// dispatch answer.
fn dispatch_answer(a: tur_engine::RutEntryAnswer) -> DispatchAnswer {
    match a {
        tur_engine::RutEntryAnswer::Nil => DispatchAnswer::Nil,
        tur_engine::RutEntryAnswer::Str(s) => DispatchAnswer::Str(s),
        tur_engine::RutEntryAnswer::Bytes(b) => DispatchAnswer::Bytes(b),
        other => panic!("unexpected dispatch answer: {other:?}"),
    }
}

fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

// ---------------------------------------------------------------------------
// The spike: seal → store → invoke, raw str and bytes.
// ---------------------------------------------------------------------------

#[test]
fn a_sealed_closure_stores_and_invokes_with_raw_str() {
    let client = boot();
    let reply = runtime()
        .block_on(client.call_host("storage:list", json!({ "dir": "/" })))
        .expect("the bound handler answers");
    assert_eq!(
        reply,
        json!({ "listed": { "dir": "/" }, "by": "hello" }),
        "the sealed closure invoked with the raw args str, its capture (db.greeting) crossed"
    );
}

#[test]
fn a_sealed_closure_invokes_with_raw_bytes() {
    let client = boot();
    let payload = b"\x00\x01lyric bytes\xff".to_vec();
    let reply = runtime()
        .block_on(client.call_host_bytes("lyric:parse", payload.clone()))
        .expect("the bound binary handler answers")
        .expect("the handler answered the payload");
    assert_eq!(
        reply, payload,
        "raw bytes rode the binary lane — no base64 anywhere"
    );
}

#[test]
fn nil_answer_is_the_not_mine_arm() {
    let client = boot();
    let reply = runtime()
        .block_on(client.call_host_bytes("lyric:parse:other", b"x".to_vec()))
        .expect("dispatch answered");
    assert!(
        reply.is_none(),
        "the nil return is the host-visible not-mine arm"
    );
    // The str lane too: an op with no bound handler dispatches to the nil
    // arm — NOT an unknown-op error (the host's lyric loop falls through
    // to the next parser on exactly this).
    let value = runtime()
        .block_on(client.call_host("no:such-op", json!({})))
        .expect("dispatch answered");
    assert_eq!(value, serde_json::Value::Null);
}

#[test]
fn a_handler_trap_surfaces_the_prefix_law() {
    let client = boot();
    let err = runtime()
        .block_on(client.call_host("fail:unauthorized", json!({})))
        .expect_err("the panic propagates as the error arm");
    match err {
        RpcError::Handler(msg) => {
            assert_eq!(
                msg, "UNAUTHORIZED: token expired",
                "the trap message keeps its prefix — classify_error reads the head"
            );
        }
        other => panic!("expected Handler, got {other:?}"),
    }
}

#[test]
fn events_ride_the_bind_table() {
    let client = boot();
    runtime().block_on(client.emit_event("music:play", json!({ "musicId": "7" })));
    // The subscriber recorded the payload into its captured ctx — read it
    // back through a bound op (the closures share the record).
    let reply = runtime()
        .block_on(client.call_host("test:last-event", json!({})))
        .expect("dispatch ok");
    assert_eq!(
        reply,
        json!({ "musicId": "7" }),
        "the event payload reached the subscriber's capture"
    );
}

// ---------------------------------------------------------------------------
// The identity law.
// ---------------------------------------------------------------------------

#[test]
fn a_forged_handle_never_binds() {
    let client = boot();
    let err = runtime()
        .block_on(client.call_host("test:forge", json!({})))
        .expect_err("the script-minted handle must trap at the row");
    match err {
        RpcError::Handler(msg) => {
            assert!(
                msg.contains("emp:rpc: foreign handle opaque"),
                "the trap must name the foreign-opaque law, got: {msg}"
            );
        }
        other => panic!("expected Handler, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// The probe-op reservation (the phase-5 cleanup): the capability probe
// dispatches `__tur_rpc_probe` expecting the inert nil arm — the table
// must refuse to bind anything under it.
// ---------------------------------------------------------------------------

#[test]
fn the_probe_op_name_is_reserved_at_bind_time() {
    let client = boot();
    let err = runtime()
        .block_on(client.call_host("test:probe-reserved", json!({})))
        .expect_err("binding under the probe op must trap");
    match err {
        RpcError::Handler(msg) => {
            assert!(
                msg.contains("reserved probe key"),
                "the trap must name the reservation, got: {msg}"
            );
        }
        other => panic!("expected Handler, got {other:?}"),
    }
    // And the capability probe still answers the inert nil arm (the
    // reservation held — nothing is bound under the probe op).
    let value = runtime()
        .block_on(client.call_host("__tur_rpc_probe", json!({})))
        .expect("the probe dispatch stays inert");
    assert_eq!(value, serde_json::Value::Null);
}
