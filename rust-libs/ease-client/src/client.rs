use std::sync::Arc;

use misty_vm::{App, AsyncRuntime};

use crate::{
    actions::Action,
    to_host::{
        connector::{ConnectorHostService, IConnectorHost},
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
    IPermissionService, PermissionService,
};

pub fn build_client(
    backend: Arc<dyn IConnectorHost>,
    permission: Arc<dyn IPermissionService>,
    router: Arc<dyn IRouterService>,
    toast: Arc<dyn IToastService>,
    vs: Arc<dyn IViewStateService>,
    async_runtime: Arc<AsyncRuntime>,
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
                .add(PermissionService::new_with_arc(permission))
                .add(RouterService::new_with_arc(router))
                .add(ConnectorHostService::new_with_arc(backend))
                .add(ToastService::new_with_arc(toast))
                .add(ViewStateService::new_with_arc(vs));
        })
        .with_async_runtime(async_runtime)
        .build()
}
