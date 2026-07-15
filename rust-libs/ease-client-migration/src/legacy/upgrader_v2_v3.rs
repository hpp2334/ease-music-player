use std::sync::Arc;

use redb::{
    MultimapTableDefinition, ReadableMultimapTable, ReadableTable, TableDefinition,
    WriteTransaction,
};

use super::v2 as legacy_v2;
use super::v3 as legacy_v3;
use ease_client_schema::v3;

use legacy_v3::{
    TABLE_BLOB, TABLE_ID_ALLOC, TABLE_MUSIC, TABLE_MUSIC_BY_LOC, TABLE_MUSIC_PLAYLIST,
    TABLE_PLAYLIST, TABLE_PLAYLIST_MUSIC, TABLE_PREFERENCE, TABLE_SCHEMA_VERSION, TABLE_STORAGE,
    TABLE_STORAGE_MUSIC,
};

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
        convert_table(db, legacy_v2::TABLE_ID_ALLOC, TABLE_ID_ALLOC)?;
        convert_table(db, legacy_v2::TABLE_PLAYLIST, TABLE_PLAYLIST)?;
        convert_multi_table(db, legacy_v2::TABLE_PLAYLIST_MUSIC, TABLE_PLAYLIST_MUSIC)?;
        convert_multi_table(db, legacy_v2::TABLE_MUSIC_PLAYLIST, TABLE_MUSIC_PLAYLIST)?;
        convert_table(db, legacy_v2::TABLE_MUSIC, TABLE_MUSIC)?;
        convert_table(db, legacy_v2::TABLE_MUSIC_BY_LOC, TABLE_MUSIC_BY_LOC)?;
        convert_table(db, legacy_v2::TABLE_STORAGE, TABLE_STORAGE)?;
        convert_multi_table(db, legacy_v2::TABLE_STORAGE_MUSIC, TABLE_STORAGE_MUSIC)?;
        convert_table(db, legacy_v2::TABLE_PREFERENCE, TABLE_PREFERENCE)?;
        convert_table(db, legacy_v2::TABLE_BLOB, TABLE_BLOB)?;
        tracing::info!("v2 -> v3: finish to upgrade to postcard");
    }
    {
        db.delete_table(legacy_v2::TABLE_ID_ALLOC)?;
        db.delete_table(legacy_v2::TABLE_PLAYLIST)?;
        db.delete_multimap_table(legacy_v2::TABLE_PLAYLIST_MUSIC)?;
        db.delete_multimap_table(legacy_v2::TABLE_MUSIC_PLAYLIST)?;
        db.delete_table(legacy_v2::TABLE_MUSIC)?;
        db.delete_table(legacy_v2::TABLE_MUSIC_BY_LOC)?;
        db.delete_table(legacy_v2::TABLE_STORAGE)?;
        db.delete_multimap_table(legacy_v2::TABLE_STORAGE_MUSIC)?;
        db.delete_table(legacy_v2::TABLE_PREFERENCE)?;
        db.delete_table(legacy_v2::TABLE_BLOB)?;
        tracing::info!("v2 -> v3: finish to delete old tables");
    }
    {
        let mut t = db.open_table(TABLE_PLAYLIST)?;
        let mut list: Vec<(v3::PlaylistId, v3::PlaylistModel)> = Default::default();
        for v in t.iter()? {
            let (k, v) = v?;
            list.push((k.value(), v.value()));
        }
        list.sort_by_key(|v| v.0);
        for (i, (id, mut model)) in list.into_iter().enumerate() {
            model.order = vec![(i + 1) as u32];
            let _ = i;
            t.insert(&id, &model)?;
        }
        tracing::info!("v2 -> v3: finish to initialize playlist order");
    }
    {
        let mut t = db.open_table(TABLE_MUSIC)?;
        let mut list: Vec<(v3::MusicId, v3::MusicModel)> = Default::default();
        for v in t.iter()? {
            let (k, v) = v?;
            list.push((k.value(), v.value()));
        }
        list.sort_by_key(|v| v.0);
        for (i, (id, mut model)) in list.into_iter().enumerate() {
            model.order = vec![(i + 1) as u32];
            let _ = i;
            t.insert(&id, &model)?;
        }
        tracing::info!("v2 -> v3: finish to initialize music order");
    }

    {
        let mut t = db.open_table(TABLE_SCHEMA_VERSION)?;
        t.insert((), 3)?;
    }
    db.commit()?;
    tracing::info!("v2 -> v3: finish all");

    Ok(())
}
