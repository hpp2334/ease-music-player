use serde::Serialize;

use crate::backends::{
    music::MusicId,
    playlist::PlaylistId,
    storage::{
        ArgUpsertStorage, StorageConnectionTestResult, StorageEntryType, StorageId, StorageType,
    },
};

#[derive(Debug, Default, Clone, Copy, Serialize, PartialEq, Eq, uniffi::Enum)]
pub enum CurrentStorageImportType {
    #[default]
    None,
    ImportMusics {
        id: PlaylistId,
    },
    EditPlaylistCover,
    CreatePlaylistEntries,
    CreatePlaylistCover,
    CurrentMusicLyrics {
        id: MusicId,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum CurrentStorageStateType {
    #[default]
    Loading,
    OK,
    NeedPermission,
    AuthenticationFailed,
    Timeout,
    UnknownError,
}
