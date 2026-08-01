//! Integration test for `ease-js-storage`: a stub JS storage provider serves
//! `list` and `get`, and the Rust `JsStorageBackend` reads them through the
//! `ease-tur-rpc` layer — proving the full storage-provider-over-RPC chain.

use std::rc::Rc;

use boa_engine::context::time::StdClock;
use ease_js_storage::JsStorageBackend;
use ease_remote_storage::StorageBackend;
use ease_tur_rpc::TurRpcPlugin;
use tur_engine::{TurRuntime, TurStdPlugin};
use tur_native::NativeFontLoader;

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

fn build_runtime() -> Rc<TurRuntime> {
    TurRuntime::builder()
        .font_loader(Rc::new(NativeFontLoader::new()))
        .clock(Rc::new(StdClock::new()))
        .plugin(TurStdPlugin)
        .plugin(TurRpcPlugin)
        .build()
        .expect("runtime")
}

/// Run `fut` on the tokio runtime while pumping engine frames on this thread
/// until it resolves. Returns the future's output.
fn run_with_pump<R: Send + 'static>(
    app: &Rc<tur_engine::TurApp>,
    rt: &tokio::runtime::Runtime,
    fut: impl std::future::Future<Output = R> + Send + 'static,
) -> R {
    let (tx, rx) = std::sync::mpsc::channel::<R>();
    rt.spawn(async move {
        let _ = tx.send(fut.await);
    });
    let mut tries = 0u32;
    loop {
        let _ = app.run_frame();
        match rx.try_recv() {
            Ok(v) => return v,
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                tries += 1;
                if tries > 20_000 {
                    panic!("storage round-trip timed out after {tries} frames");
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                panic!("storage task died")
            }
        }
    }
}

#[test]
fn js_backend_list_and_get() {
    let tokio_rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("tokio runtime");

    let runtime = build_runtime();
    let app = runtime.create_headless_app((0.0, 0.0)).expect("headless app");
    let client = ease_tur_rpc::RpcClient::of(&app).expect("rpc client");
    app.load_module(STUB_JS).expect("load js");

    let backend = JsStorageBackend::new(client, "stub", tokio_rt.handle().clone());

    // list
    let b1 = backend.clone();
    let entries = run_with_pump(&app, &tokio_rt, async move {
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
    let bytes = run_with_pump(&app, &tokio_rt, async move {
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
