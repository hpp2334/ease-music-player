use std::sync::Arc;

use ease_client_shared::backends::{
    music::MusicId,
    music_duration::MusicDuration,
    playlist::PlaylistId,
    storage::{BlobId, StorageEntryLoc},
};
use redb::{ReadTransaction, ReadableMultimapTable, ReadableTable, WriteTransaction};

use crate::{
    error::BResult,
    models::{key::DbKeyAlloc, music::MusicModel},
};

use super::{
    bin::BinSerde,
    core::DatabaseServer,
    defs::{TABLE_MUSIC, TABLE_MUSIC_BY_LOC, TABLE_PLAYLIST_MUSIC, TABLE_STORAGE_MUSIC},
};

#[derive(Debug)]
pub struct ArgDBAddMusic {
    pub loc: StorageEntryLoc,
    pub title: String,
}

impl DatabaseServer {
    pub fn load_musics_by_playlist_id(
        self: &Arc<Self>,
        playlist_id: PlaylistId,
    ) -> BResult<Vec<MusicModel>> {
        let db = self.db().begin_read()?;
        let table_playlist_musics = db.open_multimap_table(TABLE_PLAYLIST_MUSIC)?;
        let table_music = db.open_table(TABLE_MUSIC)?;
        let mut iter = table_playlist_musics.get(playlist_id)?;
        let mut ret: Vec<MusicModel> = Vec::new();
        ret.reserve(iter.len() as usize);

        while let Some(item) = iter.next() {
            let id = item?.value();

            let music = table_music.get(id)?.unwrap().value();
            ret.push(music);
        }
        Ok(ret)
    }

    pub fn load_music(self: &Arc<Self>, id: MusicId) -> BResult<Option<MusicModel>> {
        let db = self.db().begin_read()?;
        self.load_music_impl(&db, id)
    }

    pub fn load_music_impl(
        self: &Arc<Self>,
        db: &ReadTransaction,
        id: MusicId,
    ) -> BResult<Option<MusicModel>> {
        let table_music = db.open_table(TABLE_MUSIC)?;
        let model = table_music.get(id)?.map(|v| v.value()).clone();

        Ok(model)
    }

    fn load_music_by_key_impl(
        self: &Arc<Self>,
        db: &ReadTransaction,
        loc: StorageEntryLoc,
    ) -> BResult<Option<MusicModel>> {
        let id = {
            let table = db.open_table(TABLE_MUSIC_BY_LOC)?;
            table.get(loc)?.map(|v| v.value())
        };

        if let Some(id) = id {
            self.load_music_impl(db, id)
        } else {
            Ok(None)
        }
    }

    pub fn add_music_impl(
        self: &Arc<Self>,
        db: &WriteTransaction,
        rdb: &ReadTransaction,
        arg: ArgDBAddMusic,
    ) -> BResult<(MusicId, bool)> {
        let music = self.load_music_by_key_impl(rdb, arg.loc.clone())?;
        if let Some(music) = music {
            return Ok((music.id, true));
        }

        let id = self.alloc_id(db, DbKeyAlloc::Music)?;
        let id = MusicId::wrap(id);
        let mut table_music = db.open_table(TABLE_MUSIC)?;
        let mut table_music_by_loc = db.open_table(TABLE_MUSIC_BY_LOC)?;
        let mut table_storage_music = db.open_multimap_table(TABLE_STORAGE_MUSIC)?;
        table_music.insert(
            id,
            MusicModel {
                id,
                loc: arg.loc.clone(),
                title: arg.title,
                duration: None,
                cover: None,
                lyric: None,
                lyric_default: true,
            },
        )?;
        table_storage_music.insert(arg.loc.storage_id, id)?;
        table_music_by_loc.insert(arg.loc, id)?;

        return Ok((id, false));
    }

    pub fn update_music_total_duration(
        self: &Arc<Self>,
        id: MusicId,
        duration: MusicDuration,
    ) -> BResult<()> {
        let db = self.db().begin_write()?;
        {
            let mut table = db.open_table(TABLE_MUSIC)?;
            let m = table.get(id)?.map(|v| v.value());

            if let Some(mut m) = m {
                m.duration = Some(duration);
                table.insert(id, m)?;
            }
        }
        db.commit()?;

        Ok(())
    }

    pub fn update_music_cover(self: &Arc<Self>, id: MusicId, cover: Vec<u8>) -> BResult<()> {
        let db = self.db().begin_write()?;
        {
            let mut table_music = db.open_table(TABLE_MUSIC)?;
            let m = table_music.get(id)?.map(|v| v.value());

            if let Some(mut m) = m {
                if let Some(id) = m.cover {
                    self.blob().remove(id)?;
                }

                let cover_id = self.blob().write(cover)?;

                m.cover = Some(cover_id);
                table_music.insert(id, m)?;
            }
        }
        db.commit()?;

        Ok(())
    }

    pub fn update_music_lyric(
        self: &Arc<Self>,
        id: MusicId,
        loc: Option<StorageEntryLoc>,
    ) -> BResult<()> {
        let db = self.db().begin_write()?;
        {
            let mut table_music = db.open_table(TABLE_MUSIC)?;
            let m = table_music.get(id)?.map(|v| v.value());

            if let Some(mut m) = m {
                m.lyric = loc;
                m.lyric_default = false;
                table_music.insert(id, m)?;
            }
        }
        db.commit()?;

        Ok(())
    }

    pub fn compact_music_impl(
        self: &Arc<Self>,
        db: &WriteTransaction,
        rdb: &ReadTransaction,
        table_mp: &mut redb::MultimapTable<'_, BinSerde<MusicId>, BinSerde<PlaylistId>>,
        to_remove_blobs: &mut Vec<BlobId>,
        id: MusicId,
    ) -> BResult<()> {
        let ref_playlists = table_mp.get(id)?.len();

        if ref_playlists == 0 {
            let m = self.load_music_impl(&rdb, id)?.unwrap();

            let mut table_loc = db.open_table(TABLE_MUSIC_BY_LOC)?;
            let mut table_storage = db.open_multimap_table(TABLE_STORAGE_MUSIC)?;
            let mut table_m = db.open_table(TABLE_MUSIC)?;
            table_storage.remove(m.loc.storage_id, m.id)?;
            table_loc.remove(m.loc)?;
            table_m.remove(m.id)?;
            if let Some(id) = m.cover {
                to_remove_blobs.push(id);
            }
        }
        Ok(())
    }
}
