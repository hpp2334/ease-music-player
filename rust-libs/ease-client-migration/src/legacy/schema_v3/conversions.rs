use crate::legacy::{schema_v2 as v2, schema_v3 as v3};

impl From<v2::DbKeyAlloc> for v3::DbKeyAlloc {
    fn from(v2: v2::DbKeyAlloc) -> Self {
        match v2 {
            v2::DbKeyAlloc::Playlist => v3::DbKeyAlloc::Playlist,
            v2::DbKeyAlloc::Music => v3::DbKeyAlloc::Music,
            v2::DbKeyAlloc::Storage => v3::DbKeyAlloc::Storage,
        }
    }
}

impl From<v2::PlaylistModel> for v3::PlaylistModel {
    fn from(value: v2::PlaylistModel) -> Self {
        Self {
            id: value.id,
            title: value.title,
            created_time: value.created_time,
            picture: value.picture,
            order: Default::default(),
        }
    }
}

impl From<v2::MusicModel> for v3::MusicModel {
    fn from(value: v2::MusicModel) -> Self {
        Self {
            id: value.id,
            loc: value.loc,
            title: value.title,
            duration: value.duration.map(|v| v.0),
            cover: value.cover,
            lyric: value.lyric,
            lyric_default: value.lyric_default,
            order: Default::default(),
        }
    }
}

impl From<v2::StorageModel> for v3::StorageModel {
    fn from(value: v2::StorageModel) -> Self {
        Self {
            id: value.id,
            addr: value.addr,
            alias: value.alias,
            username: value.username,
            password: value.password,
            is_anonymous: value.is_anonymous,
            typ: value.typ,
        }
    }
}

impl From<v2::PreferenceModel> for v3::PreferenceModel {
    fn from(value: v2::PreferenceModel) -> Self {
        Self {
            playmode: value.playmode,
        }
    }
}
