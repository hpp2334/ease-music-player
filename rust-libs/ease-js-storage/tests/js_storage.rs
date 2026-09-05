//! Integration test for `ease-js-storage`: a stub JS storage provider serves
//! `list` and `get`, and the Rust `JsStorageBackend` reads them through the
//! `ease-tur-rpc` layer — proving the full storage-provider-over-RPC chain
//! (event bus → worker flush → JS dispatcher → reply → router → oneshot),
//! including the host-scoped `hostRpc.registerStream` path with credit-gated
//! streaming and the `dataOffset` prefix-drop for servers that ignore Range
//! requests.
//!
//! The ops are contract literals (`storage:list` / `storage:get`) — the two
//! backends multiplex on the `storageId` payload field, exactly like a real
//! multi-instance plugin.

use std::sync::{Arc, Mutex};

use ease_js_storage::JsStorageBackend;
use ease_remote_storage::StorageBackend;
use ease_tur_rpc::TurRpcPlugin;
use tur_engine::core::plugin::Plugin;
use tur_integration_tests::TurTestApp;

const STUB_JS: &str = r#"
import { hostRpc } from "tur:rpc";
import { encodeUtf8 } from "tur:std";

// plain-object body yielding one string per next() — the tur:net body shape.
function bodyOf(parts) {
  let i = 0;
  return {
    [Symbol.asyncIterator]() { return this; },
    next() {
      if (i < parts.length) {
        const v = parts[i];
        i += 1;
        return Promise.resolve({ done: false, value: encodeUtf8(v) });
      }
      return Promise.resolve({ done: true });
    },
  };
}

// last observed identity payload — asserted from Rust after each call
let lastIdentity = null;

hostRpc.registerHandler("storage:list", (args) => {
  lastIdentity = { pluginId: args.pluginId, storageId: args.storageId };
  return [
    { name: "song.flac", path: args.dir + "/song.flac", size: 2048, isDir: false },
    { name: "sub", path: args.dir + "/sub", isDir: true },
  ];
});

hostRpc.registerStream("storage:get", (args) => {
  lastIdentity = { pluginId: args.pluginId, storageId: args.storageId };
  if (args.storageId === "stubfull:test") {
    // range-IGNORING get: full 200 body from byte 0 — dataOffset: 0 tells
    // the host to drop the `offset` prefix bytes itself
    return { meta: { totalLength: 12, dataOffset: 0 }, body: bodyOf(["hello ", "world!"]) };
  }
  // range-honored get: chunks start at the requested offset
  return {
    meta: { totalLength: 12, name: "song.flac", contentType: "audio/flac" },
    body: bodyOf(["hello ", "world!"]),
  };
});

hostRpc.registerHandler("probe.lastIdentity", () => lastIdentity);
"#;

fn build_app() -> TurTestApp {
    let extra: Vec<Box<dyn Plugin>> = vec![Box::new(TurRpcPlugin)];
    TurTestApp::new_with_extra_plugins(200.0, 100.0, extra).expect("test app")
}

/// Run `fut` on a tokio runtime while pumping engine frames on the test thread
/// until it resolves. Returns the future's output.
fn run_with_pump<R: Send + 'static>(
    app: &mut TurTestApp,
    rt: &tokio::runtime::Runtime,
    fut: impl std::future::Future<Output = R> + Send + 'static,
) -> R {
    let slot: Arc<Mutex<Option<R>>> = Arc::new(Mutex::new(None));
    let slot_for_task = slot.clone();
    rt.spawn(async move {
        *slot_for_task.lock().unwrap() = Some(fut.await);
    });
    app.wait_for(|_| slot.lock().unwrap().is_some());
    let out = slot.lock().unwrap().take().expect("result settled");
    out
}

#[test]
fn js_backend_list_and_get() {
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");

    let mut app = build_app();
    let client = ease_tur_rpc::RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STUB_JS).expect("load js");

    let backend =
        JsStorageBackend::new(client.clone(), "com.ease.stub", "stub:test", tokio_rt.handle().clone());

    // list
    let b1 = backend.clone();
    let entries = run_with_pump(&mut app, &tokio_rt, async move {
        b1.list("/music".into()).await.expect("list")
    });
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "song.flac");
    assert_eq!(entries[0].path, "/music/song.flac");
    assert_eq!(entries[0].size, Some(2048));
    assert!(!entries[0].is_dir);
    assert!(entries[1].is_dir);

    // the identity payload must arrive verbatim — pluginId + storageId, no
    // composition anywhere
    let c = client.clone();
    let ident = run_with_pump(&mut app, &tokio_rt, async move {
        c.call_host("probe.lastIdentity", serde_json::json!(null))
            .await
            .expect("probe")
    });
    assert_eq!(
        ident,
        serde_json::json!({ "pluginId": "com.ease.stub", "storageId": "stub:test" }),
        "identity payload must round-trip verbatim"
    );

    // get → fully buffer the chunks via StreamFile::bytes()
    let b2 = backend.clone();
    let bytes = run_with_pump(&mut app, &tokio_rt, async move {
        b2.get("/music/song.flac".into(), 0)
            .await
            .expect("get")
            .bytes()
            .await
            .expect("bytes")
    });
    assert_eq!(bytes.as_ref(), b"hello world!");
    assert_eq!(bytes.len(), 12);
}

/// A server that ignores the Range request streams the whole body from byte 0
/// and reports `dataOffset: 0` — the backend must drop the prefix so the
/// delivered bytes start at the requested offset.
#[test]
fn js_backend_get_drops_prefix_when_range_ignored() {
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");

    let mut app = build_app();
    let client = ease_tur_rpc::RpcClient::wire(app.app()).expect("rpc client");
    app.eval_module_source(STUB_JS).expect("load js");

    let backend =
        JsStorageBackend::new(client, "com.ease.stubfull", "stubfull:test", tokio_rt.handle().clone());

    let bytes = run_with_pump(&mut app, &tokio_rt, async move {
        backend
            .get("/music/song.flac".into(), 6)
            .await
            .expect("get")
            .bytes()
            .await
            .expect("bytes")
    });
    assert_eq!(bytes.as_ref(), b"world!", "prefix bytes must be dropped");
}
