use misty_vm::{AppBuilderContext, IToHost, Model, ViewModel, ViewModelContext};
use views::{
    models::RootViewModelState,
    music::{current_music_lyric_vs, current_music_vs, time_to_pause_vs},
    playlist::{create_playlist_vs, current_playlist_vs, edit_playlist_vs, playlist_list_vs},
    router::root_subkey_vs,
    storage::{current_storage_entries_vs, edit_storage_vs, storage_list_vs},
};

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    to_host::view_state::ViewStateService,
};

use super::{
    connector::state::ConnectorState,
    main::state::RouterState,
    music::state::{CurrentMusicState, TimeToPauseState},
    playlist::state::{
        AllPlaylistState, CreatePlaylistState, CurrentPlaylistState, EditPlaylistState,
    },
    storage::state::{AllStorageState, CurrentStorageState, EditStorageState},
};

pub mod views;

macro_rules! vsb {
    ($self:expr, $cx:expr, $root:expr, $model:expr, $view_fn:expr) => {{
        let v0 = $cx.model_dirty(&$model);
        if v0 {
            let state = $cx.model_get(&$model);
            $view_fn(&state, $root);
        }
    }};
    ($self:expr, $cx:expr, $root:expr, $model1:expr, $model2:expr, $view_fn:expr) => {{
        let v0 = $cx.model_dirty(&$model1);
        let v1 = $cx.model_dirty(&$model2);
        if v0 || v1 {
            let s0 = $cx.model_get(&$model1);
            let s1 = $cx.model_get(&$model2);
            $view_fn((&s0, &s1), $root);
        }
    }};
}

pub struct ViewStateVM {
    connector: Model<ConnectorState>,
    // Music
    current_music: Model<CurrentMusicState>,
    time_to_pause: Model<TimeToPauseState>,
    // Playlist
    all_playlist: Model<AllPlaylistState>,
    current_playlist: Model<CurrentPlaylistState>,
    edit_playlist: Model<EditPlaylistState>,
    create_playlist: Model<CreatePlaylistState>,
    // Main
    router: Model<RouterState>,
    // Storage
    all_storage: Model<AllStorageState>,
    current_storage: Model<CurrentStorageState>,
    edit_storage: Model<EditStorageState>,
}

impl ViewStateVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            connector: cx.model(),
            current_music: cx.model(),
            time_to_pause: cx.model(),
            all_playlist: cx.model(),
            current_playlist: cx.model(),
            edit_playlist: cx.model(),
            create_playlist: cx.model(),
            router: cx.model(),
            all_storage: cx.model(),
            current_storage: cx.model(),
            edit_storage: cx.model(),
        }
    }

    fn notify(&self, cx: &ViewModelContext) {
        let mut root: RootViewModelState = Default::default();
        self.notify_view_state_builders_impl(cx, &mut root);
        ViewStateService::of(cx).handle_notify(root);
    }

    fn notify_view_state_builders_impl(
        &self,
        cx: &ViewModelContext,
        root: &mut RootViewModelState,
    ) {
        // Music
        vsb!(self, cx, root, self.current_music, current_music_vs);
        vsb!(self, cx, root, self.time_to_pause, time_to_pause_vs);
        vsb!(self, cx, root, self.current_music, current_music_lyric_vs);
        // Playlist
        vsb!(
            self,
            cx,
            root,
            self.all_playlist,
            self.connector,
            playlist_list_vs
        );
        vsb!(
            self,
            cx,
            root,
            self.current_playlist,
            self.connector,
            current_playlist_vs
        );
        vsb!(
            self,
            cx,
            root,
            self.edit_playlist,
            self.connector,
            edit_playlist_vs
        );
        vsb!(
            self,
            cx,
            root,
            self.create_playlist,
            self.connector,
            create_playlist_vs
        );
        // Main
        vsb!(self, cx, root, self.router, root_subkey_vs);
        // Storage
        vsb!(self, cx, root, self.all_storage, storage_list_vs);
        vsb!(
            self,
            cx,
            root,
            self.current_storage,
            self.all_storage,
            current_storage_entries_vs
        );
        vsb!(self, cx, root, self.edit_storage, edit_storage_vs);
    }
}

impl ViewModel for ViewStateVM {
    type Event = Action;
    type Error = EaseError;

    fn on_event(&self, cx: &ViewModelContext, _event: &Action) -> EaseResult<()> {
        self.notify(cx);
        Ok(())
    }

    fn on_flush(&self, cx: &ViewModelContext) -> Result<(), Self::Error> {
        self.notify(cx);
        Ok(())
    }
}
