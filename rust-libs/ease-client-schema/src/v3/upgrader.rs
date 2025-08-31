use std::sync::Arc;

use redb::{
    MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
    WriteTransaction,
};

use crate::{v2, v3};

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
            id: value.id.into(),
            title: value.title,
            created_time: value.created_time,
            picture: value.picture.map(|p| p.into()),
            order: Default::default(),
        }
    }
}

impl From<v2::MusicModel> for v3::MusicModel {
    fn from(value: v2::MusicModel) -> Self {
        Self {
            id: value.id.into(),
            loc: value.loc.into(),
            title: value.title,
            duration: value.duration.map(|v| v.0),
            cover: value.cover.map(|c| c.into()),
            lyric: value.lyric.map(|l| l.into()),
            lyric_default: value.lyric_default,
            order: Default::default(),
        }
    }
}

impl From<v2::StorageModel> for v3::StorageModel {
    fn from(value: v2::StorageModel) -> Self {
        Self {
            id: value.id.into(),
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

fn convert_table<KF, VF, KT, VT>(
    db: &WriteTransaction,
    d_from: TableDefinition<KF, VF>,
    d_to: TableDefinition<KT, VT>,
) -> anyhow::Result<()>
where
    KF: redb::Key + 'static,
    VF: redb::Value + 'static,
    KT: redb::Key + 'static,
    VT: redb::Value + 'static,
    for<'b> <KT as redb::Value>::SelfType<'b>: From<<KF as redb::Value>::SelfType<'b>>,
    for<'b> <VT as redb::Value>::SelfType<'b>: From<<VF as redb::Value>::SelfType<'b>>,
{
    let ot = db.open_table(d_from)?;
    let mut nt = db.open_table(d_to)?;
    for v in ot.iter()? {
        let v = v?;
        nt.insert(&v.0.value().into(), &v.1.value().into())?;
    }
    Ok(())
}

fn convert_multi_table<KF, VF, KT, VT>(
    db: &WriteTransaction,
    d_from: MultimapTableDefinition<KF, VF>,
    d_to: MultimapTableDefinition<KT, VT>,
) -> anyhow::Result<()>
where
    KF: redb::Key + 'static,
    VF: redb::Key + 'static,
    KT: redb::Key + 'static,
    VT: redb::Key + 'static,
    for<'b> <KT as redb::Value>::SelfType<'b>: From<<KF as redb::Value>::SelfType<'b>>,
    for<'b> <VT as redb::Value>::SelfType<'b>: From<<VF as redb::Value>::SelfType<'b>>,
{
    let ot = db.open_multimap_table(d_from)?;
    let mut nt = db.open_multimap_table(d_to)?;

    for v in ot.iter()? {
        let (k, v) = v?;

        for v in v.into_iter() {
            let v = v?;
            nt.insert(&k.value().into(), &v.value().into())?;
        }
    }
    Ok(())
}

pub fn upgrade_v2_to_v3(database: &Arc<redb::Database>) -> anyhow::Result<()> {
    let db = database.begin_write()?;
    {
        let ref db = db;
        convert_table(db, v2::TABLE_ID_ALLOC, v3::TABLE_ID_ALLOC)?;
        convert_table(db, v2::TABLE_PLAYLIST, v3::TABLE_PLAYLIST)?;
        convert_multi_table(db, v2::TABLE_PLAYLIST_MUSIC, v3::TABLE_PLAYLIST_MUSIC)?;
        convert_multi_table(db, v2::TABLE_MUSIC_PLAYLIST, v3::TABLE_MUSIC_PLAYLIST)?;
        convert_table(db, v2::TABLE_MUSIC, v3::TABLE_MUSIC)?;
        convert_table(db, v2::TABLE_MUSIC_BY_LOC, v3::TABLE_MUSIC_BY_LOC)?;
        convert_table(db, v2::TABLE_STORAGE, v3::TABLE_STORAGE)?;
        convert_multi_table(db, v2::TABLE_STORAGE_MUSIC, v3::TABLE_STORAGE_MUSIC)?;
        convert_table(db, v2::TABLE_PREFERENCE, v3::TABLE_PREFERENCE)?;
        convert_table(db, v2::TABLE_BLOB, v3::TABLE_BLOB)?;
        tracing::info!("v2 -> v3: finish to upgrade to postcard");
    }
    {
        db.delete_table(v2::TABLE_ID_ALLOC)?;
        db.delete_table(v2::TABLE_PLAYLIST)?;
        db.delete_multimap_table(v2::TABLE_PLAYLIST_MUSIC)?;
        db.delete_multimap_table(v2::TABLE_MUSIC_PLAYLIST)?;
        db.delete_table(v2::TABLE_MUSIC)?;
        db.delete_table(v2::TABLE_MUSIC_BY_LOC)?;
        db.delete_table(v2::TABLE_STORAGE)?;
        db.delete_multimap_table(v2::TABLE_STORAGE_MUSIC)?;
        db.delete_table(v2::TABLE_PREFERENCE)?;
        db.delete_table(v2::TABLE_BLOB)?;
        tracing::info!("v2 -> v3: finish to delete old tables");
    }
    {
        let mut t = db.open_table(v3::TABLE_PLAYLIST)?;
        let mut list: Vec<(v3::PlaylistId, v3::PlaylistModel)> = Default::default();
        for v in t.iter()? {
            let (k, v) = v?;
            list.push((k.value(), v.value().clone()));
        }
        list.sort_by_key(|v| v.0);
        for (i, (id, mut model)) in list.into_iter().enumerate() {
            model.order = vec![(i + 1) as u32];
            t.insert(&id, &model)?;
        }
        tracing::info!("v2 -> v3: finish to initialize playlist order");
    }
    {
        let mut t = db.open_table(v3::TABLE_MUSIC)?;
        let mut list: Vec<(v3::MusicId, v3::MusicModel)> = Default::default();
        for v in t.iter()? {
            let (k, v) = v?;
            list.push((k.value(), v.value().clone()));
        }
        list.sort_by_key(|v| v.0);
        for (i, (id, mut model)) in list.into_iter().enumerate() {
            model.order = vec![(i + 1) as u32];
            t.insert(&id, &model)?;
        }
        tracing::info!("v2 -> v3: finish to initialize music order");
    }

    {
        let mut t = db.open_table(v3::TABLE_SCHEMA_VERSION)?;
        t.insert((), 3)?;
    }
    db.commit()?;
    tracing::info!("v2 -> v3: finish all");

    Ok(())
}
