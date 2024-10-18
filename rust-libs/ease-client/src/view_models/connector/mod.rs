
use ease_client_backend::error::BResult;
use ease_client_shared::backends::{
    music::{ArgUpdateMusicLyric, Music, MusicId},
    playlist::{Playlist, PlaylistId},
    storage::{ListStorageEntryChildrenResp, StorageEntryLoc},
};
use misty_vm::{ViewModel, ViewModelContext};

use crate::{actions::Action, error::{EaseError, EaseResult}};

pub struct Connector {}

impl Connector {
    pub async fn get_music(&self, id: MusicId) -> BResult<Option<Music>> {
        todo!()
    }

    pub async fn get_playlist(&self, id: PlaylistId) -> BResult<Option<Playlist>> {
        todo!()
    }

    pub async fn update_music_lyric(&self, arg: ArgUpdateMusicLyric) -> BResult<()> {
        todo!()
    }

    pub async fn list_storage_entry_children(
        &self,
        loc: StorageEntryLoc,
    ) -> BResult<ListStorageEntryChildrenResp> {
        todo!()
    }
}


impl ViewModel<Action, EaseError> for Connector {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        Ok(())
    }
}