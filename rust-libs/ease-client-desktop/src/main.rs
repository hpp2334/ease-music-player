use core::{assets::Assets, view_state::{GpuiViewStateService, ViewStates}, vm::{build_desktop_backend, build_desktop_client, build_lifecycle, AppBridge}};

use ease_client::{
    Action, AppPod,
};

use ease_client_shared::backends::{app::ArgInitializeApp, playlist::PlaylistId, storage::{Storage, StorageType}};
use futures::{channel::mpsc, StreamExt};
use gpui::prelude::*;
use gpui::{
    px, size, App, AppContext, Bounds,
    WindowBounds, WindowOptions,
};
use misty_lifecycle::Runnable;
use tracing::level_filters::LevelFilter;
use views::{base::input_base::setup_input_keyboards, root::RootComponent};

pub mod views;
pub mod core;


fn patch_cwd() {
    let cwd = std::env::current_dir().unwrap();
    if cwd.ends_with("rust-libs") {
        std::env::set_current_dir(cwd.join("ease-client-desktop")).unwrap();
    }
    println!("CWD: {:?}", std::env::current_dir());
}


fn setup_subscriber() {
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(LevelFilter::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber).unwrap();
}

fn main() {
    setup_subscriber();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();

    patch_cwd();

    App::new()
        .with_assets(Assets {})
        .run(|cx: &mut AppContext| {
            let (foreground_sender, mut foreground_receiver) = mpsc::channel::<Runnable>(128);
            let vs = ViewStates::new(cx);

            setup_input_keyboards(cx);

            cx.spawn(|cx| async move {
                while let Some(runnable) = foreground_receiver.next().await {
                    runnable.run();

                    let _ = cx.update(|cx| {
                        cx.global::<AppBridge>().clone().flush(cx);
                    });
                }
            })
            .detach();

            {
                let lifecycle_external = build_lifecycle(cx, foreground_sender);
                let backend = build_desktop_backend(lifecycle_external.clone());
                backend
                    .init(ArgInitializeApp {
                        app_document_dir: "./temp/".to_string(),
                        app_cache_dir: "./temp/".to_string(),
                        storage_path: "/home/a/".to_string(),
                    })
                    .unwrap();

                let app = build_desktop_client(lifecycle_external.clone(), backend, vs.clone());
                app.dispatch(cx, Action::Init);
                app.dispatch(cx, Action::VsLoaded);

                cx.set_global(app);
            }

            let bounds = Bounds::centered(None, size(px(1280.0 + 32.0), px(800.0 + 32.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: None,
                    window_background: gpui::WindowBackgroundAppearance::Transparent,
                    ..Default::default()
                },
                |cx| cx.new_view(|cx| RootComponent::new(cx, &vs)),
            )
            .unwrap();
        });
}
