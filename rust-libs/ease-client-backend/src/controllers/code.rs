use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Code {
    // Playlist
    GetAllPlaylistMetas,
    GetPlaylist,
    UpdatePlaylist,
    AddMusicsToPlaylist,
    RemoveMusicFromPlaylist,
    RemovePlaylist,
}
