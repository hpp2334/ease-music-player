use std::sync::Arc;

use ease_client_backend::error::BResult;
use ease_client_shared::backends::{
    music::{ArgUpdateMusicLyric, Music, MusicId},
    playlist::{Playlist, PlaylistId},
};
use misty_vm::ViewModelContext;

pub struct Connector {}
impl Connector {
    pub fn of(cx: &ViewModelContext) -> Arc<Connector> {
        todo!()
    }

    pub async fn get_music(&self, id: MusicId) -> BResult<Option<Music>> {
        todo!()
    }

    pub async fn get_playlist(&self, id: PlaylistId) -> BResult<Option<Playlist>> {
        todo!()
    }

    pub async fn update_music_lyric(&self, arg: ArgUpdateMusicLyric) -> BResult<()> {
        todo!()
    }
}
