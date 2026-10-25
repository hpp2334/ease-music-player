//! Integration test for `ease-js-storage`: a rut storage-provider fixture
//! serves `storage:list` / `storage:get` / `storage:get-chunk` /
//! `storage:release` through the bus rows, and the Rust `JsStorageBackend`
//! reads them through the `ease-tur-rpc` layer — proving the full
//! storage-over-RPC chain (chunked pull, the base64 chunk crossing, the
//! envelope decode, and the `dataOffset` prefix-drop for servers that
//! ignore Range requests).
//!
//! The ops are contract literals; the fixture multiplexes on the
//! `storageId` payload field like a real multi-instance plugin. The payload
//! base64 is a precomputed constant (the harness has no `ease.crypto`
//! rows — the real plugins encode over those).

use ease_js_storage::JsStorageBackend;
use ease_remote_storage::StorageBackend;
use ease_tur_rpc::{RpcBus, RpcClient, RutDriver, TurRpcBusPlugin};
use std::sync::mpsc;
use std::sync::Arc;
use tur_integration_tests::TurTestApp;

const PAYLOAD: &[u8] = b"hello world!";
const PAYLOAD_B64: &str = "aGVsbG8gd29ybGQh";

/// The storage fixture: the file `/music/song.flac` (PAYLOAD) + a
/// directory. `storage:get` opens session 7 with `dataOffset: 0` — the
/// range-IGNORING server shape, so the host must drop the requested-offset
/// prefix itself; `storage:get-chunk` answers the whole payload in one
/// chunk with `eof`.
const STORE_RUT: &str = r#"
use tur_host::ease::rpc::{ register, take_request };

struct Store {
    payload_b64: str,
    size: u64,
}

entry fn start() -> opaque {
    register("storage:list", "op_list");
    register("storage:get", "op_get");
    register("storage:get-chunk", "op_chunk");
    register("storage:release", "op_release");
    return opaque(Store { payload_b64: "__B64__", size: __SIZE__ });
}

entry fn op_list(cx: opaque) -> str {
    let s = opaque::downcast<Store>(cx);
    if (s == nil) {
        panic("list: cx is not a Store");
    }
    let req = take_request();
    let _ = req;
    return f"{{\"ok\":true,\"result\":[{{\"name\":\"song.flac\",\"path\":\"/music/song.flac\",\"size\":{s.size},\"isDir\":false,\"createdAt\":1700000000000,\"modifiedAt\":1700000001000}},{{\"name\":\"sub\",\"path\":\"/music/sub\",\"isDir\":true}}]}}";
}

// The range-ignoring opener: dataOffset 0 (chunks start at byte 0 even
// though the host asked for `offset`).
entry fn op_get(cx: opaque) -> str {
    let s = opaque::downcast<Store>(cx);
    if (s == nil) {
        panic("get: cx is not a Store");
    }
    return f"{{\"ok\":true,\"result\":{{\"sid\":7,\"meta\":{{\"totalLength\":{s.size},\"name\":\"song.flac\",\"contentType\":\"audio/flac\",\"dataOffset\":0}}}}}}";
}

// One chunk: the whole payload, then eof.
entry fn op_chunk(cx: opaque) -> str {
    let s = opaque::downcast<Store>(cx);
    if (s == nil) {
        panic("chunk: cx is not a Store");
    }
    let req = take_request();
    let _ = req;
    return f"{{\"ok\":true,\"result\":{{\"data\":\"{s.payload_b64}\",\"eof\":true}}}}";
}

entry fn op_release(cx: opaque) -> str {
    let _ = opaque::downcast<Store>(cx);
    return "{\"ok\":true,\"result\":true}";
}
"#;

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

/// Boot the storage fixture (the app stays on the driver thread).
fn boot() -> (RpcClient, tokio::runtime::Runtime) {
    let bus = RpcBus::new();
    let (tx, rx) = mpsc::channel::<Cmd>();
    let (ready_tx, ready_rx) = mpsc::channel::<()>();
    let bus_for_plugin = Some(bus.clone());
    let fixture = STORE_RUT
        .replace("__B64__", PAYLOAD_B64)
        .replace("__SIZE__", &PAYLOAD.len().to_string());
    std::thread::spawn(move || {
        let app = TurTestApp::new_with_extra_plugins(
            400.0,
            400.0,
            vec![Box::new(TurRpcBusPlugin::new(bus_for_plugin))],
        )
        .expect("harness app");
        app.load_rut_module(&fixture).expect("fixture loads");
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
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");
    (
        RpcClient::new(bus, Arc::new(TestDriver { tx })),
        tokio_rt,
    )
}

#[test]
fn list_roundtrips_entries_with_identity_payload() {
    let (client, tokio_rt) = boot();
    let backend = JsStorageBackend::new(
        client,
        "com.ease.stub",
        "stub:test",
        tokio_rt.handle().clone(),
    );
    let entries = tokio_rt.block_on(backend.list("/music".into())).expect("list");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "song.flac");
    assert_eq!(entries[0].size, Some(PAYLOAD.len()));
    assert!(!entries[0].is_dir);
    assert_eq!(entries[0].created_at, Some(1_700_000_000_000));
    assert!(entries[1].is_dir);
}

#[test]
fn get_pulls_chunks_into_a_stream_file() {
    let (client, tokio_rt) = boot();
    let backend = JsStorageBackend::new(
        client,
        "com.ease.stub",
        "stub:test",
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
/// requested offset.
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
        .block_on(backend.get("/music/song.flac".into(), 6))
        .expect("get");
    let bytes = tokio_rt.block_on(file.bytes()).expect("bytes");
    assert_eq!(bytes.as_ref(), b"world!", "prefix bytes must be dropped");
}
