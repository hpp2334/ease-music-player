//! Integration test for `ease-js-storage`: a rut storage-provider fixture
//! serves `storage:list` / `storage:get` / `storage:get-chunk` /
//! `storage:release` through the bind-table rows, and the Rust
//! `JsStorageBackend` reads them through the `ease-tur-rpc` layer —
//! proving the full storage-over-RPC chain (chunked pull, the base64
//! chunk crossing, the envelope decode, and the `dataOffset` prefix-drop
//! for servers that ignore Range requests).
//!
//! The ops are contract literals; the fixture multiplexes on the
//! `storageId` payload field like a real multi-instance plugin. The payload
//! base64 is a precomputed constant (the harness has no `ease.crypto`
//! rows — the real plugins encode over those).
//!
//! Phase 5 made the bind table the only contract — the old stash-bus
//! fixture is gone; this suite is the transport coverage.

use ease_js_storage::JsStorageBackend;
use ease_remote_storage::StorageBackend;
use ease_tur_rpc::{
    DispatchAnswer, RpcClient, RutDriver, TurRpcBindPlugin,
};
use std::sync::mpsc;
use std::sync::Arc;
use tur_integration_tests::TurTestApp;

const PAYLOAD: &[u8] = b"hello world!";
const PAYLOAD_B64: &str = "aGVsbG8gd29ybGQh";

/// The storage fixture: the file `/music/song.flac` (PAYLOAD). The opener
/// answers `dataOffset: 0` — the range-IGNORING server shape, so the host
/// must drop the requested-offset prefix itself; `storage:get-chunk`
/// answers the whole payload in one chunk with `eof`.
const STORE_RUT: &str = r#"
use rpc_host::rpc::{ bind, handler };
use json::{ JsonReader };

let PAYLOAD_B64: str = "__B64__";
let SIZE: u64 = __SIZE__;

// the dispatch entries (declared once per backend) — two-line forwards
// over the rows, the `emp::Rpc` shape without the SDK
entry fn rpc(cx: opaque, op: str, args: str) -> ?str {
    let b = handler(cx, op);
    if (b == nil) {
        return nil;
    }
    let f = opaque::downcast<fn(str) -> ?str>(b);
    return f(args);
}

// the plain fns (each decodes its OWN args with the json lib)
fn op_list(args: str) -> ?str {
    let dir = read_str_field(args, "dir");
    return f"[{{\"name\":\"song.flac\",\"path\":\"{dir}/song.flac\",\"size\":{SIZE},\"isDir\":false,\"createdAt\":1700000000000,\"modifiedAt\":1700000001000}}]";
}

fn op_get(args: str) -> ?str {
    let _ = read_str_field(args, "path");
    return f"{{\"sid\":7,\"meta\":{{\"totalLength\":{SIZE},\"name\":\"song.flac\",\"contentType\":\"audio/flac\",\"dataOffset\":0}}}}";
}

fn op_chunk(args: str) -> ?str {
    let _ = read_i64_field(args, "sid");
    let _ = read_i64_field(args, "offset");
    return f"{{\"data\":\"{PAYLOAD_B64}\",\"eof\":true}}";
}

fn op_release(args: str) -> ?str {
    let _ = read_i64_field(args, "sid");
    return "true";
}

// the json-field decoders (the `json` std lib)
fn read_str_field(args: str, want: str) -> str {
    let r = JsonReader::of(args);
    let out: str = "";
    r.begin_object();
    while (r.more()) {
        let k = r.next_key();
        if (k == want) {
            out = r.read_str();
        } else {
            r.skip_value();
        }
    }
    r.end_object();
    return out;
}

fn read_i64_field(args: str, want: str) -> i64 {
    let r = JsonReader::of(args);
    let out: i64 = 0;
    r.begin_object();
    while (r.more()) {
        let k = r.next_key();
        if (k == want) {
            out = r.read_i64();
        } else {
            r.skip_value();
        }
    }
    r.end_object();
    return out;
}

entry fn start(cx: opaque) {
    let _ = cx;
    bind(cx, "storage:list", opaque(fn (args: str) -> ?str { return op_list(args); }));
    bind(cx, "storage:get", opaque(fn (args: str) -> ?str { return op_get(args); }));
    bind(cx, "storage:get-chunk", opaque(fn (args: str) -> ?str { return op_chunk(args); }));
    bind(cx, "storage:release", opaque(fn (args: str) -> ?str { return op_release(args); }));
}
"#;

enum Cmd {
    DispatchStr(
        String,
        u64,
        String,
        String,
        mpsc::SyncSender<Result<DispatchAnswer, String>>,
    ),
    DispatchBytes(
        String,
        u64,
        String,
        Vec<u8>,
        mpsc::SyncSender<Result<DispatchAnswer, String>>,
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
        let (rtx, rrx) = mpsc::sync_channel(1);
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
        let (rtx, rrx) = mpsc::sync_channel(1);
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
    tx: mpsc::Sender<Cmd>,
}

/// Boot the bind-table fixture (stamped with a fixed identity; loaded
/// through the cx lane).
fn boot() -> (RpcClient, tokio::runtime::Runtime) {
    const BOOT_CX: u64 = 4040;
    let (tx, rx) = mpsc::channel::<Cmd>();
    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let fixture = STORE_RUT
        .replace("__B64__", PAYLOAD_B64)
        .replace("__SIZE__", &PAYLOAD.len().to_string());
    std::thread::spawn(move || {
        let app = TurTestApp::new_with_extra_plugins(
            400.0,
            400.0,
            vec![
                // The standalone bind rows + the identity stamp + the
                // sealed-handler table.
                Box::new(TurRpcBindPlugin::with_identity(BOOT_CX)),
            ],
        )
        .expect("harness app");
        app.load_rut_module_cx(&fixture, BOOT_CX)
            .expect("fixture loads");
        ready_tx.send(()).expect("test still waiting");
        for cmd in rx {
            match cmd {
                Cmd::DispatchStr(name, cx, op, args, rtx) => {
                    let _ = rtx.send(
                        app.call_rut_entry_cx_str_str(&name, cx, &op, &args)
                            .map(|a| match a {
                                tur_engine::RutEntryAnswer::Nil => DispatchAnswer::Nil,
                                tur_engine::RutEntryAnswer::Str(s) => DispatchAnswer::Str(s),
                                tur_engine::RutEntryAnswer::Bytes(b) => DispatchAnswer::Bytes(b),
                                other => panic!("unexpected dispatch answer: {other:?}"),
                            })
                            .map_err(|e| e.to_string()),
                    );
                }
                Cmd::DispatchBytes(name, cx, op, args, rtx) => {
                    let _ = rtx.send(
                        app.call_rut_entry_cx_str_bytes(&name, cx, &op, &args)
                            .map(|a| match a {
                                tur_engine::RutEntryAnswer::Nil => DispatchAnswer::Nil,
                                tur_engine::RutEntryAnswer::Str(s) => DispatchAnswer::Str(s),
                                tur_engine::RutEntryAnswer::Bytes(b) => DispatchAnswer::Bytes(b),
                                other => panic!("unexpected dispatch answer: {other:?}"),
                            })
                            .map_err(|e| e.to_string()),
                    );
                }
            }
        }
    });
    ready_rx.recv().expect("driver thread booted");
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    (
        RpcClient::new(Arc::new(TestDriver { tx }), BOOT_CX),
        tokio_rt,
    )
}

#[test]
fn list_roundtrips_over_the_bind_table_lane() {
    let (client, tokio_rt) = boot();
    let backend = JsStorageBackend::new(
        client,
        "com.ease.stub2",
        "stub2:test",
        tokio_rt.handle().clone(),
    );
    let entries = tokio_rt.block_on(backend.list("/music".into())).expect("list");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "song.flac");
    assert_eq!(entries[0].size, Some(PAYLOAD.len()));
    assert!(!entries[0].is_dir);
}

#[test]
fn get_pulls_chunks_over_the_bind_table_lane() {
    let (client, tokio_rt) = boot();
    let backend = JsStorageBackend::new(
        client,
        "com.ease.stub2",
        "stub2:test",
        tokio_rt.handle().clone(),
    );
    // The opener answers dataOffset: 0 — the prefix-drop path too.
    let file = tokio_rt
        .block_on(backend.get("/music/song.flac".into(), 6))
        .expect("get");
    let bytes = tokio_rt.block_on(file.bytes()).expect("bytes");
    assert_eq!(bytes.as_ref(), b"world!", "prefix bytes must be dropped");
}

#[test]
fn get_surfaces_the_file_shape() {
    let (client, tokio_rt) = boot();
    let backend = JsStorageBackend::new(
        client,
        "com.ease.stub2",
        "stub2:test",
        tokio_rt.handle().clone(),
    );
    let file = tokio_rt
        .block_on(backend.get("/music/song.flac".into(), 0))
        .expect("get");
    assert_eq!(file.name(), "song.flac");
    assert_eq!(file.total_size(), Some(PAYLOAD.len()));
    assert_eq!(file.content_type().unwrap(), "audio/flac");
    let bytes = tokio_rt.block_on(file.bytes()).expect("bytes");
    assert_eq!(bytes.as_ref(), PAYLOAD);
}

/// A server that ignores the Range request answers `dataOffset: 0` — the
/// backend must drop the prefix so the delivered bytes start at the
/// requested offset (the last byte's offset — the drop must land exactly).
#[test]
fn get_drops_prefix_when_range_ignored() {
    let (client, tokio_rt) = boot();
    let backend = JsStorageBackend::new(
        client,
        "com.ease.stubfull",
        "stubfull:test",
        tokio_rt.handle().clone(),
    );
    let file = tokio_rt
        .block_on(backend.get("/music/song.flac".into(), 11))
        .expect("get");
    let bytes = tokio_rt.block_on(file.bytes()).expect("bytes");
    assert_eq!(bytes.as_ref(), b"!", "prefix bytes must be dropped");
}
