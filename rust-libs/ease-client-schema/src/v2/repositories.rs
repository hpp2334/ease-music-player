use std::fmt::Debug;

use super::models::*;
use super::shared::*;
use redb::{MultimapTableDefinition, TableDefinition, TypeName};

#[derive(Debug)]
pub struct BinSerde<T>(T);

trait BinSerdeTN {
    const NAME: &'static str;
}

impl<T> redb::Value for BinSerde<T>
where
    T: Debug + BinSerdeTN + bitcode::Encode + for<'a> bitcode::Decode<'a>,
{
    type SelfType<'a>
        = T
    where
        Self: 'a;

    type AsBytes<'a>
        = Vec<u8>
    where
        Self: 'a;

    fn fixed_width() -> Option<usize> {
        None
    }

    fn from_bytes<'a>(data: &'a [u8]) -> Self::SelfType<'a>
    where
        Self: 'a,
    {
        bitcode::decode(data).unwrap()
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        bitcode::encode(value)
    }

    fn type_name() -> TypeName {
        TypeName::new(&format!("BinSerde<{}>", std::any::type_name::<T>()))
    }
}

impl<T> redb::Key for BinSerde<T>
where
    T: Debug + BinSerdeTN + bitcode::Encode + bitcode::DecodeOwned + Ord,
{
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        <Self as redb::Value>::from_bytes(data1).cmp(&<Self as redb::Value>::from_bytes(data2))
    }
}

impl BinSerdeTN for DbKeyAlloc {
    const NAME: &'static str = "";
}
impl BinSerdeTN for PlaylistId {
    const NAME: &'static str = "";
}

impl BinSerdeTN for MusicId {
    const NAME: &'static str = "";
}

impl BinSerdeTN for StorageEntryLoc {
    const NAME: &'static str = "";
}

impl BinSerdeTN for StorageId {
    const NAME: &'static str = "";
}

impl BinSerdeTN for BlobId {
    const NAME: &'static str = "";
}

impl BinSerdeTN for PlaylistModel {
    const NAME: &'static str = "";
}

impl BinSerdeTN for MusicModel {
    const NAME: &'static str = "";
}

impl BinSerdeTN for StorageModel {
    const NAME: &'static str = "";
}

impl BinSerdeTN for PreferenceModel {
    const NAME: &'static str = "";
}

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
pub const TABLE_BLOB: TableDefinition<(), BinSerde<BlobId>> = TableDefinition::new("blob");
