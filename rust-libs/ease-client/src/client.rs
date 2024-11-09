use std::sync::{atomic::AtomicBool, Arc};

use ease_client_shared::backends::app::ArgInitializeApp;
use misty_vm::{App, AppPod, IAsyncRuntimeAdapter};
use once_cell::sync::Lazy;

use crate::{
    actions::{event::ViewAction, Action},
    async_adapter::AsyncAdapter,
    to_host::{
        connector::{ConnectorHostImpl, ConnectorHostService},
        flush_notifier::IFlushNotifier,
        player::{IMusicPlayerService, MusicPlayerService},
        router::{IRouterService, RouterService},
        toast::{IToastService, ToastService},
        view_state::{IViewStateService, ViewStateService},
    },
    view_models::{
        connector::Connector,
        main::{router::RouterVM, MainBodyVM},
        music::{
            common::MusicCommonVM, control::MusicControlVM, detail::MusicDetailVM,
            lyric::MusicLyricVM, time_to_pause::TimeToPauseVM,
        },
        playlist::{
            common::PlaylistCommonVM, create::PlaylistCreateVM, detail::PlaylistDetailVM,
            edit::PlaylistEditVM, list::PlaylistListVM,
        },
        storage::{
            common::StorageCommonVM, import::StorageImportVM, list::StorageListVM,
            upsert::StorageUpsertVM,
        },
        view_state::ViewStateVM,
    },
};
use tracing::subscriber::set_global_default;

static CLIENT: Lazy<AppPod> = Lazy::new(|| AppPod::new());
static ASYNC_RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(4)
        .build()
        .unwrap()
});

fn create_log(dir: &str) -> std::fs::File {
    let p = std::path::Path::new(dir).join("latest.log");
    let _r = std::fs::remove_file(&p);
    let file = std::fs::File::create(&p).unwrap();
    file
}

fn trace_level() -> tracing::Level {
    if std::env::var("EBUILD").is_ok() {
        tracing::Level::INFO
    } else {
        tracing::Level::TRACE
    }
}

#[cfg(target_os = "android")]
fn setup_subscriber(dir: &str) {
    use tracing_subscriber::layer::SubscriberExt;
    let log_file = create_log(dir);
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(trace_level())
        .with_writer(log_file)
        .with_ansi(false)
        .finish();
    let subscriber = subscriber.with(tracing_android::layer("com.ease_music_player").unwrap());
    set_global_default(subscriber).unwrap();
}

#[cfg(not(target_os = "android"))]
fn setup_subscriber(dir: &str) {
    let log_file = create_log(dir);
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(trace_level())
        .with_writer(log_file)
        .with_ansi(false)
        .finish();

    set_global_default(subscriber).unwrap();
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let stacktrace = std::backtrace::Backtrace::force_capture();

        tracing::error!("panic info: {}", info);
        tracing::error!("panic stacktrace: {}", stacktrace);

        std::process::abort()
    }));
}

fn init_tracers(dir: &str) {
    static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);
    let is_init = IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst);
    std::env::set_var("RUST_BACKTRACE", "1");
    if !is_init {
        setup_subscriber(dir);
        setup_panic_hook();
    }
}

pub fn build_client(
    router: Arc<dyn IRouterService>,
    player: Arc<dyn IMusicPlayerService>,
    toast: Arc<dyn IToastService>,
    vs: Arc<dyn IViewStateService>,
    adapter: impl IAsyncRuntimeAdapter,
) -> App {
    App::builder::<Action>()
        .with_view_models(|cx, builder| {
            // Connector
            builder.add(Connector::new(cx));
            // Storage
            builder.add(StorageCommonVM::new(cx));
            builder.add(StorageImportVM::new(cx));
            builder.add(StorageListVM::new(cx));
            builder.add(StorageUpsertVM::new(cx));
            // Playlist
            builder.add(PlaylistCommonVM::new(cx));
            builder.add(PlaylistListVM::new(cx));
            builder.add(PlaylistCreateVM::new(cx));
            builder.add(PlaylistDetailVM::new(cx));
            builder.add(PlaylistEditVM::new(cx));
            // Music
            builder.add(MusicCommonVM::new(cx));
            builder.add(MusicControlVM::new(cx));
            builder.add(MusicDetailVM::new(cx));
            builder.add(MusicLyricVM::new(cx));
            builder.add(TimeToPauseVM::new(cx));
            // Main
            builder.add(RouterVM::new(cx));
            builder.add(MainBodyVM::new(cx));
            // View State
            builder.add(ViewStateVM::new(cx));
        })
        .with_to_hosts(|builder| {
            builder
                .add(RouterService::new_with_arc(router))
                .add(ConnectorHostService::new(ConnectorHostImpl::new()))
                .add(MusicPlayerService::new_with_arc(player))
                .add(ToastService::new_with_arc(toast))
                .add(ViewStateService::new_with_arc(vs));
        })
        .with_async_runtime_adapter(adapter)
        .build()
}

#[uniffi::export]
pub fn api_build_client(
    router: Arc<dyn IRouterService>,
    player: Arc<dyn IMusicPlayerService>,
    toast: Arc<dyn IToastService>,
    vs: Arc<dyn IViewStateService>,
    notifier: Arc<dyn IFlushNotifier>,
) {
    let _guard = ASYNC_RT.enter();
    let app = build_client(router, player, toast, vs, AsyncAdapter::new(notifier));
    CLIENT.set(app);
}

#[uniffi::export]
pub fn api_start_client(arg: ArgInitializeApp) {
    let _guard = ASYNC_RT.enter();
    init_tracers(&arg.app_document_dir);
    CLIENT.get().emit(Action::Init(arg));
}

#[uniffi::export]
pub fn api_emit_view_action(action: ViewAction) {
    let _guard = ASYNC_RT.enter();
    CLIENT.get().emit(Action::View(action));
}

#[uniffi::export]
pub fn api_flush_spawned() {
    let _guard = ASYNC_RT.enter();
    CLIENT.get().flush_spawned();
}
