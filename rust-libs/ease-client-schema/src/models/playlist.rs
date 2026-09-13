use serde::{Deserialize, Serialize};

use crate::shared::{MusicId, PlaylistGroupId, PlaylistId, StorageEntryLoc, StorageId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistModel {
    pub id: PlaylistId,
    pub title: String,
    pub created_time: i64,
    pub picture: Option<StorageEntryLoc>,
    pub order: Vec<u32>,
    /// Import-source restriction: `None` = all storages may be imported
    /// from, `Some(ids)` = only the listed storages. Purely an import
    /// picker restriction — it never filters the playlist's existing
    /// musics or playback.
    pub storage_allowlist: Option<Vec<StorageId>>,
    /// Owning group. `None` only transiently (pre-group databases) —
    /// the ensure-default sweep assigns orphans to the first group.
    pub group_id: Option<PlaylistGroupId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistMusicModel {
    pub playlist_id: PlaylistId,
    pub music_id: MusicId,
}
