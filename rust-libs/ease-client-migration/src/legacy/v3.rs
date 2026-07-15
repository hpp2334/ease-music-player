use std::fmt::Debug;

use ease_client_schema::v3::{
    BlobId, DbKeyAlloc, MusicId, PlaylistId, StorageEntryLoc, StorageId, MusicModel,
    PlaylistModel, PreferenceModel, StorageModel,
};
use redb::{MultimapTableDefinition, TableDefinition, TypeName};

pub trait BinSerdeTN {
    const NAME: &'static str;
}

#[derive(Debug)]
pub struct BinSerde<T>(pub T);

impl<T> redb::Value for BinSerde<T>
where
    T: Debug + serde::Serialize + BinSerdeTN + for<'a> serde::Deserialize<'a>,
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
        postcard::from_bytes(data).unwrap()
    }

    fn as_bytes<'a, 'b: 'a>(value: &'a Self::SelfType<'b>) -> Self::AsBytes<'a>
    where
        Self: 'a,
        Self: 'b,
    {
        postcard::to_allocvec(value).unwrap()
    }

    fn type_name() -> TypeName {
        TypeName::new(&format!("BinSerdeV3<{}>", T::NAME))
    }
}

impl<T> redb::Key for BinSerde<T>
where
    T: Debug + serde::Serialize + BinSerdeTN + for<'a> serde::Deserialize<'a> + Ord,
{
    fn compare(data1: &[u8], data2: &[u8]) -> std::cmp::Ordering {
        <Self as redb::Value>::from_bytes(data1).cmp(&<Self as redb::Value>::from_bytes(data2))
    }
}

impl BinSerdeTN for DbKeyAlloc {
    const NAME: &'static str = "DbKeyAlloc";
}
impl BinSerdeTN for PlaylistId {
    const NAME: &'static str = "PlaylistId";
}
impl BinSerdeTN for MusicId {
    const NAME: &'static str = "MusicId";
}
impl BinSerdeTN for StorageId {
    const NAME: &'static str = "StorageId";
}
impl BinSerdeTN for BlobId {
    const NAME: &'static str = "BlobId";
}
impl BinSerdeTN for StorageEntryLoc {
    const NAME: &'static str = "StorageEntryLoc";
}
impl BinSerdeTN for MusicModel {
    const NAME: &'static str = "MusicModel";
}
impl BinSerdeTN for PlaylistModel {
    const NAME: &'static str = "PlaylistModel";
}
impl BinSerdeTN for PreferenceModel {
    const NAME: &'static str = "PreferenceModel";
}
impl BinSerdeTN for StorageModel {
    const NAME: &'static str = "StorageModel";
}

pub const TABLE_ID_ALLOC: TableDefinition<BinSerde<DbKeyAlloc>, i64> =
    TableDefinition::new("v3_alloc");
pub const TABLE_PLAYLIST: TableDefinition<BinSerde<PlaylistId>, BinSerde<PlaylistModel>> =
    TableDefinition::new("v3_playlist");
pub const TABLE_PLAYLIST_MUSIC: MultimapTableDefinition<BinSerde<PlaylistId>, BinSerde<MusicId>> =
    MultimapTableDefinition::new("v3_playlist_music");
pub const TABLE_MUSIC_PLAYLIST: MultimapTableDefinition<BinSerde<MusicId>, BinSerde<PlaylistId>> =
    MultimapTableDefinition::new("v3_music_playlist");
pub const TABLE_MUSIC: TableDefinition<BinSerde<MusicId>, BinSerde<MusicModel>> =
    TableDefinition::new("v3_music");
pub const TABLE_MUSIC_BY_LOC: TableDefinition<BinSerde<StorageEntryLoc>, BinSerde<MusicId>> =
    TableDefinition::new("v3_music_by_loc");
pub const TABLE_STORAGE: TableDefinition<BinSerde<StorageId>, BinSerde<StorageModel>> =
    TableDefinition::new("v3_storage");
pub const TABLE_STORAGE_MUSIC: MultimapTableDefinition<BinSerde<StorageId>, BinSerde<MusicId>> =
    MultimapTableDefinition::new("v3_storage_music");
pub const TABLE_PREFERENCE: TableDefinition<(), BinSerde<PreferenceModel>> =
    TableDefinition::new("v3_preference");
pub const TABLE_SCHEMA_VERSION: TableDefinition<(), u32> = TableDefinition::new("schema_version");
pub const TABLE_BLOB: TableDefinition<(), BinSerde<BlobId>> = TableDefinition::new("v3_blob");
