use ease_client_shared::backends::{
    music::MusicId,
    playlist::PlaylistId,
    storage::{BlobId, StorageEntryLoc, StorageId},
};
use redb::{MultimapTableDefinition, TableDefinition};

use crate::models::{
    key::DbKeyAlloc, music::MusicModel, playlist::PlaylistModel, storage::StorageModel,
};

use super::{bin::BinSerde, preference::PreferenceModel};

pub const TABLE_ID_ALLOC: TableDefinition<BinSerde<DbKeyAlloc>, i64> =
    TableDefinition::new("alloc");
pub const TABLE_PLAYLIST: TableDefinition<BinSerde<PlaylistId>, BinSerde<PlaylistModel>> =
    TableDefinition::new("playlist");
pub const TABLE_PLAYLIST_MUSIC: MultimapTableDefinition<BinSerde<PlaylistId>, BinSerde<MusicId>> =
    MultimapTableDefinition::new("playlist_music");
pub const TABLE_MUSIC: TableDefinition<BinSerde<MusicId>, BinSerde<MusicModel>> =
    TableDefinition::new("music");
pub const TABLE_MUSIC_BY_LOC: TableDefinition<BinSerde<StorageEntryLoc>, BinSerde<MusicId>> =
    TableDefinition::new("music_by_loc");
pub const TABLE_STORAGE: TableDefinition<BinSerde<StorageId>, BinSerde<StorageModel>> =
    TableDefinition::new("storage");
pub const TABLE_STORAGE_MUSIC: TableDefinition<BinSerde<StorageId>, BinSerde<MusicId>> =
    TableDefinition::new("storage_music");
pub const TABLE_PREFERENCE: TableDefinition<(), BinSerde<PreferenceModel>> =
    TableDefinition::new("preference");
pub const TABLE_SCHEMA_VERSION: TableDefinition<(), u64> = TableDefinition::new("schema_version");
pub const TABLE_BLOB: TableDefinition<BinSerde<BlobId>, Vec<u8>> = TableDefinition::new("blob");
