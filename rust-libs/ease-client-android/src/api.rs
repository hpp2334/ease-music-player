use std::{
    sync::{atomic::AtomicBool, Arc},
    thread::ThreadId,
    time::{Duration, SystemTime},
};

use ease_client::{build_client, Action, ViewAction};
use ease_client_backend::Backend;
use ease_client_shared::backends::{
    app::ArgInitializeApp, encode_message_payload, generated::Code, player::PlayerDelegateEvent,
    storage::DataSourceKey, MessagePayload,
};
use misty_vm::{AsyncRuntime, BoxFuture, IAsyncRuntimeAdapter};

use tracing::subscriber::set_global_default;

use crate::{
    foreigns::{
        IAsyncAdapterForeign, IPermissionServiceForeign, IPlayerDelegateForeign,
        IRouterServiceForeign, IToastServiceForeign, IViewStateServiceForeign,
        PermissionServiceDelegate, PlayerDelegate, RouterServiceDelegate, ToastServiceDelegate,
        ViewStateServiceDelegate,
    },
    inst::{BACKEND, CLIENTS, RT},
};

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
        tracing::Level::INFO
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

#[uniffi::export]
pub fn api_build_backend(
    async_adapter: Arc<dyn IAsyncAdapterForeign>,
    player: Arc<dyn IPlayerDelegateForeign>,
) {
    let _guard = RT.enter();
    let backend = Backend::new(
        AsyncRuntime::new(AsyncAdapterDelegate::new(async_adapter, None)),
        PlayerDelegate::new(player),
    );
    BACKEND.set_backend(Arc::new(backend));
}

#[uniffi::export]
pub fn api_start_backend(arg: ArgInitializeApp) {
    let _guard = RT.enter();
    init_tracers(&arg.app_document_dir);
    BACKEND.backend().init(arg).unwrap();
}

#[uniffi::export]
pub fn api_destroy_backend() {
    let _guard = RT.enter();
    BACKEND.reset_backend();
}

#[uniffi::export]
pub async fn api_load_asset(key: DataSourceKey) -> Option<Vec<u8>> {
    RT.spawn(async move {
        if let Some(backend) = BACKEND.try_backend() {
            let file = backend.load_asset(key, 0).await;
            if let Ok(Some(file)) = file {
                return file.bytes().await.ok().map(|v| v.to_vec());
            }
        }
        None
    })
    .await
    .unwrap()
}

#[uniffi::export]
pub fn api_build_client(
    permission: Arc<dyn IPermissionServiceForeign>,
    router: Arc<dyn IRouterServiceForeign>,
    toast: Arc<dyn IToastServiceForeign>,
    vs: Arc<dyn IViewStateServiceForeign>,
    async_adapter: Arc<dyn IAsyncAdapterForeign>,
) -> u64 {
    let _guard = RT.enter();
    let app_id = CLIENTS.preallocate();
    let app = build_client(
        BACKEND.clone(),
        PermissionServiceDelegate::new(permission),
        RouterServiceDelegate::new(router),
        ToastServiceDelegate::new(toast),
        ViewStateServiceDelegate::new(vs),
        AsyncRuntime::new(AsyncAdapterDelegate::new(async_adapter, Some(app_id))),
    );
    CLIENTS.allocate(app_id, app);
    app_id
}

#[uniffi::export]
pub fn api_start_client(handle: u64) {
    let _guard = RT.enter();
    let client = CLIENTS.get(handle);
    client.emit(Action::Init);
}

#[uniffi::export]
pub fn api_destroy_client(handle: u64) {
    let _guard = RT.enter();
    let client = CLIENTS.take(handle);
    client.emit(Action::Init);
}

#[uniffi::export]
pub fn api_emit_view_action(handle: u64, action: ViewAction) {
    let _guard = RT.enter();
    let client = CLIENTS.try_get(handle);
    if let Some(client) = client {
        client.emit(Action::View(action));
    }
}

#[uniffi::export]
pub fn api_flush_spawned_locals(handle: u64) {
    let _guard = RT.enter();
    let client = CLIENTS.try_get(handle);
    if let Some(client) = client {
        client.flush_spawned();
    }
}
