//! Conversions from the migration-internal v3 schema types (which carry
//! bitcode/postcard derives for legacy redb decoding) to the public v4 types
//! in `ease_client_schema` (trimmed derives). The two type families are
//! structurally identical; these impls are field-by-field copies.

use ease_client_schema as schema;

use crate::legacy::schema_v2 as v2;
use crate::legacy::schema_v3 as v3;

impl From<v2::MusicId> for schema::MusicId {
    fn from(v: v2::MusicId) -> Self {
        schema::MusicId::wrap(*v.as_ref())
    }
}
impl From<v2::PlaylistId> for schema::PlaylistId {
    fn from(v: v2::PlaylistId) -> Self {
        schema::PlaylistId::wrap(*v.as_ref())
    }
}
impl From<v2::StorageId> for schema::StorageId {
    fn from(v: v2::StorageId) -> Self {
        schema::StorageId::wrap(*v.as_ref())
    }
}
impl From<v2::BlobId> for schema::BlobId {
    fn from(v: v2::BlobId) -> Self {
        schema::BlobId::wrap(*v.as_ref())
    }
}

// NOTE: the legacy `v2::StorageType` -> `schema::StorageType` and
// `v3::StorageModel` -> `schema::StorageModel` conversions were removed: the
// schema no longer carries a flat `StorageModel` (the `storage` table is now a
// kind-agnostic registry, with details split into `webdav_storage` /
// `secret`), and `schema::StorageType` lost the `OneDrive` variant. Legacy
// storage rows are imported directly from their v3 shape in `import_from_redb`.

impl From<v2::PlayMode> for schema::PlayMode {
    fn from(v: v2::PlayMode) -> Self {
        match v {
            v2::PlayMode::Single => schema::PlayMode::Single,
            v2::PlayMode::SingleLoop => schema::PlayMode::SingleLoop,
            v2::PlayMode::List => schema::PlayMode::List,
            v2::PlayMode::ListLoop => schema::PlayMode::ListLoop,
        }
    }
}

impl From<v2::StorageEntryLoc> for schema::StorageEntryLoc {
    fn from(v: v2::StorageEntryLoc) -> Self {
        schema::StorageEntryLoc {
            storage_id: v.storage_id.into(),
            path: v.path,
        }
    }
}

impl From<v3::DbKeyAlloc> for schema::DbKeyAlloc {
    fn from(v: v3::DbKeyAlloc) -> Self {
        match v {
            v3::DbKeyAlloc::Playlist => schema::DbKeyAlloc::Playlist,
            v3::DbKeyAlloc::Music => schema::DbKeyAlloc::Music,
            v3::DbKeyAlloc::Storage => schema::DbKeyAlloc::Storage,
        }
    }
}

impl From<v3::MusicModel> for schema::MusicModel {
    fn from(v: v3::MusicModel) -> Self {
        schema::MusicModel {
            id: v.id.into(),
            loc: v.loc.into(),
            title: v.title,
            duration: v.duration,
            cover: v.cover.map(Into::into),
            lyric: v.lyric.map(Into::into),
            lyric_default: v.lyric_default,
            order: v.order,
        }
    }
}

impl From<v3::PlaylistModel> for schema::PlaylistModel {
    fn from(v: v3::PlaylistModel) -> Self {
        schema::PlaylistModel {
            id: v.id.into(),
            title: v.title,
            created_time: v.created_time,
            picture: v.picture.map(Into::into),
            order: v.order,
        }
    }
}

impl From<v3::PreferenceModel> for schema::PreferenceModel {
    fn from(v: v3::PreferenceModel) -> Self {
        schema::PreferenceModel {
            playmode: v.playmode.into(),
            // The legacy schemas predate the language override.
            language: None,
        }
    }
}
