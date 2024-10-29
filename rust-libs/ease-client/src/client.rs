use std::sync::Arc;

use ease_client_shared::backends::app::ArgInitializeApp;
use misty_vm::{App, AppPod, IAsyncRuntimeAdapter};
use once_cell::sync::Lazy;

use crate::{
    actions::{event::ViewAction, Action}, async_adapter::AsyncAdapter, error::EaseError, to_host::{
        connector::{ConnectorHostImpl, ConnectorHostService}, flush_notifier::IFlushNotifier, player::{IMusicPlayerService, MusicPlayerService}, toast::{IToastService, ToastService}, view_state::{IViewStateService, ViewStateService}
    }, view_models::{
        connector::Connector,
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
    }
};

static CLIENT: Lazy<AppPod> = Lazy::new(|| AppPod::new());
static ASYNC_RT: Lazy<tokio::runtime::Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .unwrap()
});


pub fn build_client(
    player: Arc<dyn IMusicPlayerService>,
    toast: Arc<dyn IToastService>,
    vs: Arc<dyn IViewStateService>,
    adapter: impl IAsyncRuntimeAdapter,
) -> App {
    App::builder::<Action, EaseError>()
        .with_view_models(|cx, builder| {
            // Connector
            builder.add(Connector::new(cx));
            // Music
            builder.add(MusicCommonVM::new(cx));
            builder.add(MusicControlVM::new(cx));
            builder.add(MusicDetailVM::new(cx));
            builder.add(MusicLyricVM::new(cx));
            builder.add(TimeToPauseVM::new(cx));
            // Playlist
            builder.add(PlaylistCommonVM::new(cx));
            builder.add(PlaylistListVM::new(cx));
            builder.add(PlaylistCreateVM::new(cx));
            builder.add(PlaylistDetailVM::new(cx));
            builder.add(PlaylistEditVM::new(cx));
            // Storage
            builder.add(StorageCommonVM::new(cx));
            builder.add(StorageImportVM::new(cx));
            builder.add(StorageListVM::new(cx));
            builder.add(StorageUpsertVM::new(cx));
            // View State
            builder.add(ViewStateVM::new(cx));
        })
        .with_to_hosts(|builder| {
            builder
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
    player: Arc<dyn IMusicPlayerService>,
    toast: Arc<dyn IToastService>,
    vs: Arc<dyn IViewStateService>,
    notifier: Arc<dyn IFlushNotifier>
) {
    let app = build_client(
        player,
        toast,
        vs,
        AsyncAdapter::new(notifier)
    );
    CLIENT.set(app);
}

#[uniffi::export]
pub fn api_start_client(arg: ArgInitializeApp) {
    let _guard = ASYNC_RT.enter();
    CLIENT.get().emit::<_, EaseError>(Action::Init(arg));
}

#[uniffi::export]
pub fn api_emit_view_action(action: ViewAction) {
    let _guard = ASYNC_RT.enter();
    CLIENT.get().emit::<_, EaseError>(action);
}

#[uniffi::export]
pub fn api_flush_spawned() {
    let _guard = ASYNC_RT.enter();
    CLIENT.get().flush_spawned();
}
