use ease_client_shared::backends::app::ArgInitializeApp;
use misty_vm::{App, AppPod};
use once_cell::sync::Lazy;

use crate::{
    actions::{event::ViewAction, Action},
    error::EaseError,
    view_models::{
        connector::Connector,
        music::{
            common::MusicCommonVM, control::MusicControlVM, detail::MusicDetailVM,
            lyric::MusicLyricVM, time_to_pause::TimeToPauseVM,
        }, playlist::{common::PlaylistCommonVM, create::PlaylistCreateVM, detail::PlaylistDetailVM, edit::PlaylistEditVM, list::PlaylistListVM}, storage::{common::StorageCommonVM, import::StorageImportVM, list::StorageListVM, upsert::StorageUpsertVM},
    },
};

static CLIENT: Lazy<AppPod> = Lazy::new(|| AppPod::new());

#[uniffi::export]
pub fn api_build_client() {
    let app = App::builder::<Action, EaseError>()
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
        })
        .build();
    CLIENT.set(app);
}


#[uniffi::export]
pub fn api_start_client(arg: ArgInitializeApp) {
    CLIENT.get().emit(Action::Init(arg));
}

#[uniffi::export]
pub fn api_emit_view_action(action: ViewAction) {
    CLIENT.get().emit(action);
}
