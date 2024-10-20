
use ease_client_shared::backends::playlist::{Playlist, PlaylistId};
use misty_vm::{AppBuilderContext, Model, ViewModel, ViewModelContext};

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    view_models::connector::Connector,
};

use super::state::{AllPlaylistState, CurrentPlaylistState};

pub struct PlaylistCommonVM {
    store: Model<AllPlaylistState>,
    current: Model<CurrentPlaylistState>,
}

impl PlaylistCommonVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            store: cx.model(),
            current: cx.model(),
        }
    }

    pub(crate) fn remove(&self, cx: &ViewModelContext, id: PlaylistId) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(move |cx| async move {
            Connector::of(&cx).remove_playlist(id).await?;
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn remove_current(&self, cx: &ViewModelContext) -> EaseResult<()> {
        if let Some(id) = cx.model_get(&self.current).id {
            self.remove(cx, id)
        } else {
            Ok(())
        }
    }

    pub(crate) fn get_current(&self, cx: &ViewModelContext) -> EaseResult<Option<Playlist>> {
        let playlist = {
            let store = cx.model_get(&self.store);
            let id = cx.model_get(&self.current).id;

            if let Some(id) = id {
                store.playlists.get(&id).map(|v| v.clone())
            } else {
                None
            }
        };
        Ok(playlist)
    }
}

impl ViewModel<Action, EaseError> for PlaylistCommonVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        Ok(())
    }
}
