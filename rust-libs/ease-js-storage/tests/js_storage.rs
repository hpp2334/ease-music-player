//! Integration test for `ease-js-storage`: a stub JS storage provider serves
//! `list` and `get`, and the Rust `JsStorageBackend` reads them through the
//! `ease-tur-rpc` layer — proving the full storage-provider-over-RPC chain
//! (event bus → worker flush → JS dispatcher → reply → router → oneshot).

use std::sync::{Arc, Mutex};

use ease_js_storage::JsStorageBackend;
use ease_remote_storage::StorageBackend;
use ease_tur_rpc::TurRpcPlugin;
use tur_engine::core::plugin::Plugin;
use tur_integration_tests::TurTestApp;

const STUB_JS: &str = r#"
import { registerHandler, pushChunk, endStream } from "tur:rpc";
import { encodeUtf8 } from "tur:std";

registerHandler("stub:list", (args) => {
  return [
    { name: "song.flac", path: args.dir + "/song.flac", size: 2048, isDir: false },
    { name: "sub", path: args.dir + "/sub", isDir: true },
  ];
});

registerHandler("stub:get", (args) => {
  const sid = args.streamId;
  // echo a fixed 12-byte payload regardless of path/offset (good enough for
  // the bridge test; real providers honor offset via a Range request)
  pushChunk(sid, encodeUtf8("hello "));
  pushChunk(sid, encodeUtf8("world!"));
  endStream(sid);
  return { totalLength: 12, name: "song.flac", contentType: "audio/flac" };
});
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
        JsStorageBackend::new(client, "stub", "stub:test", tokio_rt.handle().clone());

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
