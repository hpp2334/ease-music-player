use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
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
    UpdatePreferencePlaymode,
}
