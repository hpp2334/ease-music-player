use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum Code {
    // Playlist
    GetAllPlaylistMetas,
    GetPlaylist,
    CreatePlaylist,
    UpdatePlaylist,
    AddMusicsToPlaylist,
    RemoveMusicFromPlaylist,
    RemovePlaylist,
    // Music
    GetMusic,
    UpdateMusicDuration,
    UpdateMusicCover,
    UpdateMusicLyric,
    // Storage
    UpsertStorage,
    ListStorage,
    GetStorage,
    RemoveStorage,
    TestStorage,
    ListStorageEntryChildren,
    // Preference
    GetPreference,
    UpdatePreference,
}
