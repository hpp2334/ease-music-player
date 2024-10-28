use ease_client_shared::backends::playlist::{Playlist, PlaylistAbstract, PlaylistId};
use misty_vm::{AppBuilderContext, AsyncTasks, Model, ViewModel, ViewModelContext};

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    view_models::connector::{Connector, ConnectorAction},
};

use super::state::{AllPlaylistState, CurrentPlaylistState};

pub struct PlaylistCommonVM {
    store: Model<AllPlaylistState>,
    current: Model<CurrentPlaylistState>,
    tasks: AsyncTasks,
}

impl PlaylistCommonVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            store: cx.model(),
            current: cx.model(),
            tasks: Default::default()
        }
    }

    pub(crate) fn remove(&self, cx: &ViewModelContext, id: PlaylistId) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).remove_playlist(&cx, id).await?;
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn remove_current(&self, cx: &ViewModelContext) -> EaseResult<()> {
        if let Some(playlist) = &cx.model_get(&self.current).playlist {
            self.remove(cx, playlist.id())
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_current(&self, cx: &ViewModelContext) -> EaseResult<Option<Playlist>> {
        let playlist = {
            let playlist = cx.model_get(&self.current).playlist.clone();
            playlist
        };
        Ok(playlist)
    }

    pub(crate) fn has_playlist(&self, cx: &ViewModelContext, id: PlaylistId) -> bool {
        let store = cx.model_get(&self.store);
        store.playlists.iter().any(|v| v.id() == id)
    }

    fn sync_playlists(&self, cx: &ViewModelContext, playlists: Vec<PlaylistAbstract>) {
        let mut state = cx.model_mut(&self.store);
        state.playlists = playlists;
    }

    fn sync_playlist(&self, cx: &ViewModelContext, playlist: Playlist) {
        let mut current_state = cx.model_mut(&self.current);
        current_state.playlist = Some(playlist);
    }
}

impl ViewModel<Action, EaseError> for PlaylistCommonVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::Connector(action) => match action {
                ConnectorAction::PlaylistAbstracts(playlists) => {
                    self.sync_playlists(cx, playlists.clone());
                }
                ConnectorAction::Playlist(playlist) => {
                    self.sync_playlist(cx, playlist.clone());
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
