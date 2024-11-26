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
        AssetLoadDelegate, IAssetLoadDelegateForeign, IAsyncAdapterForeign,
        IPermissionServiceForeign, IPlayerDelegateForeign, IRouterServiceForeign,
        IToastServiceForeign, IViewStateServiceForeign, PermissionServiceDelegate, PlayerDelegate,
        RouterServiceDelegate, ToastServiceDelegate, ViewStateServiceDelegate,
    },
    inst::{BACKEND, CLIENTS, RT},
};

pub struct AsyncAdapterDelegate {
    inner: Arc<dyn IAsyncAdapterForeign>,
    app_id: Option<u64>,
    thread_id: ThreadId,
}
impl AsyncAdapterDelegate {
    pub fn new(inner: Arc<dyn IAsyncAdapterForeign>, app_id: Option<u64>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            app_id,
            thread_id: std::thread::current().id(),
        })
    }
    fn app_id(&self) -> Option<u64> {
        self.app_id
    }
}
impl IAsyncRuntimeAdapter for AsyncAdapterDelegate {
    fn is_main_thread(&self) -> bool {
        std::thread::current().id() == self.thread_id
    }

    fn on_spawn_locals(&self) {
        self.inner.on_spawn_locals(self.app_id());
    }

    fn get_time(&self) -> Duration {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
    }

    fn sleep(&self, duration: Duration) -> BoxFuture<()> {
        Box::pin(tokio::time::sleep(duration))
    }
}

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
pub fn api_send_backend_player_event(evt: PlayerDelegateEvent) {
    let _guard = RT.enter();
    let backend = BACKEND.backend();
    backend.request_from_host(MessagePayload {
        code: Code::OnPlayerEvent,
        payload: encode_message_payload(evt),
    });
}

#[uniffi::export]
pub fn api_backend_play_next() {
    let _guard = RT.enter();
    let backend = BACKEND.backend();
    backend.request_from_host(MessagePayload {
        code: Code::PlayNext,
        payload: encode_message_payload(()),
    });
}

#[uniffi::export]
pub fn api_backend_play_previous() {
    let _guard = RT.enter();
    let backend = BACKEND.backend();
    backend.request_from_host(MessagePayload {
        code: Code::PlayPrevious,
        payload: encode_message_payload(()),
    });
}

#[uniffi::export]
pub fn api_flush_backend_spawned_local() {
    let _guard = RT.enter();
    if let Some(backend) = BACKEND.try_backend() {
        backend.flush_spawned_locals();
    }
}

#[uniffi::export]
pub async fn api_load_asset(key: DataSourceKey) -> Option<Vec<u8>> {
    RT.spawn(async move {
        if let Some(backend) = BACKEND.try_backend() {
            let file = backend.asset_server().load(key, 0).await;
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
pub fn api_open_asset(
    key: DataSourceKey,
    offset: u64,
    listener: Arc<dyn IAssetLoadDelegateForeign>,
) -> u64 {
    let _guard = RT.enter();
    let backend = BACKEND.backend();
    backend
        .asset_server()
        .open(key, offset as usize, AssetLoadDelegate::new(listener))
}

#[uniffi::export]
pub fn api_close_asset(id: u64) {
    let _guard = RT.enter();
    let backend = BACKEND.try_backend();
    if let Some(backend) = backend {
        backend.asset_server().close(id);
    }
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
