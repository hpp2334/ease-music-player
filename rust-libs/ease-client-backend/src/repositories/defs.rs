use ease_client_shared::backends::{
    music::MusicId,
    playlist::PlaylistId,
    storage::{StorageEntryLoc, StorageId},
};
use redb::{MultimapTableDefinition, TableDefinition};

use crate::models::{
    key::DbKeyAlloc, music::MusicModel, playlist::PlaylistModel, preference::PreferenceModel,
    storage::StorageModel,
};

use super::bin::BinSerde;

pub const TABLE_ID_ALLOC: TableDefinition<BinSerde<DbKeyAlloc>, i64> =
    TableDefinition::new("alloc");
pub const TABLE_PLAYLIST: TableDefinition<BinSerde<PlaylistId>, BinSerde<PlaylistModel>> =
    TableDefinition::new("playlist");
pub const TABLE_PLAYLIST_MUSIC: MultimapTableDefinition<BinSerde<PlaylistId>, BinSerde<MusicId>> =
    MultimapTableDefinition::new("playlist_music");
pub const TABLE_MUSIC_PLAYLIST: MultimapTableDefinition<BinSerde<MusicId>, BinSerde<PlaylistId>> =
    MultimapTableDefinition::new("music_playlist");
pub const TABLE_MUSIC: TableDefinition<BinSerde<MusicId>, BinSerde<MusicModel>> =
    TableDefinition::new("music");
pub const TABLE_MUSIC_BY_LOC: TableDefinition<BinSerde<StorageEntryLoc>, BinSerde<MusicId>> =
    TableDefinition::new("music_by_loc");
pub const TABLE_STORAGE: TableDefinition<BinSerde<StorageId>, BinSerde<StorageModel>> =
    TableDefinition::new("storage");
pub const TABLE_STORAGE_MUSIC: MultimapTableDefinition<BinSerde<StorageId>, BinSerde<MusicId>> =
    MultimapTableDefinition::new("storage_music");
pub const TABLE_PREFERENCE: TableDefinition<(), BinSerde<PreferenceModel>> =
    TableDefinition::new("preference");
pub const TABLE_SCHEMA_VERSION: TableDefinition<(), u32> = TableDefinition::new("schema_version");
