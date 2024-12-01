use std::sync::Arc;

use ease_client_shared::backends::storage::{ArgUpsertStorage, StorageId};
use redb::{ReadTransaction, ReadableMultimapTable, ReadableTable, ReadableTableMetadata};

use crate::{
    error::BResult,
    models::{key::DbKeyAlloc, storage::StorageModel},
};

use super::{
    core::DatabaseServer,
    defs::{
        TABLE_MUSIC, TABLE_MUSIC_BY_LOC, TABLE_MUSIC_PLAYLIST, TABLE_PLAYLIST_MUSIC, TABLE_STORAGE,
        TABLE_STORAGE_MUSIC,
    },
};

impl DatabaseServer {
    pub fn load_storage_music_count(self: &Arc<Self>, id: StorageId) -> BResult<u64> {
        let db = self.db().begin_read()?;
        let table = db.open_multimap_table(TABLE_STORAGE_MUSIC)?;
        let len = table.get(id)?.len();
        Ok(len)
    }

    pub fn load_storage(self: &Arc<Self>, id: StorageId) -> BResult<Option<StorageModel>> {
        let db = self.db().begin_read()?;
        self.load_storage_impl(&db, id)
    }

    fn load_storage_impl(
        self: &Arc<Self>,
        db: &ReadTransaction,
        id: StorageId,
    ) -> BResult<Option<StorageModel>> {
        let table = db.open_table(TABLE_STORAGE)?;
        let p = table.get(id)?.map(|v| v.value());
        Ok(p)
    }

    pub fn load_storages(self: &Arc<Self>) -> BResult<Vec<StorageModel>> {
        let db = self.db().begin_read()?;
        let table = db.open_table(TABLE_STORAGE)?;
        let len = table.len()? as usize;

        let mut ret: Vec<StorageModel> = Default::default();
        ret.reserve(len);

        let mut iter = table.iter()?;
        while let Some(v) = iter.next() {
            let v = v?.1.value();
            ret.push(v);
        }

        Ok(ret)
    }

    pub fn upsert_storage(self: &Arc<Self>, arg: ArgUpsertStorage) -> BResult<StorageId> {
        let db = self.db().begin_write()?;

        let id = {
            let mut table = db.open_table(TABLE_STORAGE)?;
            let mut model = if let Some(id) = arg.id {
                let v = table.get(id)?.unwrap().value();
                v
            } else {
                let id = self.alloc_id(&db, DbKeyAlloc::Storage)?;
                let v = StorageModel {
                    id: StorageId::wrap(id),
                    addr: Default::default(),
                    alias: Default::default(),
                    username: Default::default(),
                    password: Default::default(),
                    is_anonymous: Default::default(),
                    typ: Default::default(),
                };
                v
            };
            let id = model.id;

            model.addr = arg.addr;
            model.alias = arg.alias;
            model.username = arg.username;
            model.password = arg.password;
            model.is_anonymous = arg.is_anonymous;
            model.typ = arg.typ;
            table.insert(model.id, model)?;

            id
        };
        db.commit()?;

        Ok(id)
    }

    pub fn remove_storage(self: &Arc<Self>, id: StorageId) -> BResult<()> {
        let db = self.db().begin_write()?;
        let rdb = self.db().begin_read()?;

        {
            let mut table_storage_musics = db.open_multimap_table(TABLE_STORAGE_MUSIC)?;
            let mut table_playlist_musics = db.open_multimap_table(TABLE_PLAYLIST_MUSIC)?;
            let mut table_music_playlists = db.open_multimap_table(TABLE_MUSIC_PLAYLIST)?;
            let mut table_storage = db.open_table(TABLE_STORAGE)?;
            let mut table_musics = db.open_table(TABLE_MUSIC)?;
            let mut table_music_by_loc = db.open_table(TABLE_MUSIC_BY_LOC)?;

            let mut music_iter = table_storage_musics.get(id)?;

            while let Some(v) = music_iter.next() {
                let id = v?.value();

                let mut iter = table_music_playlists.get(id)?;
                while let Some(v) = iter.next() {
                    let playlist_id = v?.value();
                    table_playlist_musics.remove(playlist_id, id)?;
                }
                drop(iter);
                table_music_playlists.remove_all(id)?;

                {
                    let m = self.load_music_impl(&rdb, id)?.unwrap();
                    if let Some(id) = m.cover {
                        self.remove_blob_impl(&db, id)?;
                    }
                }

                table_musics.remove(id)?;
            }
            drop(music_iter);

            table_music_by_loc.retain(|v, _| v.storage_id != id)?;
            table_storage.remove(id)?;
            table_storage_musics.remove_all(id)?;
        }

        db.commit()?;

        Ok(())
    }
}
