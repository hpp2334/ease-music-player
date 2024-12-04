use std::sync::Arc;

use ease_client_shared::backends::{
    music::MusicId,
    playlist::PlaylistId,
    storage::{BlobId, StorageEntryLoc},
};
use redb::{ReadTransaction, ReadableMultimapTable, ReadableTable, ReadableTableMetadata};

use crate::{
    error::BResult,
    models::{key::DbKeyAlloc, playlist::PlaylistModel},
};

use super::{
    core::DatabaseServer,
    defs::{TABLE_MUSIC_PLAYLIST, TABLE_PLAYLIST, TABLE_PLAYLIST_MUSIC},
    music::ArgDBAddMusic,
};

impl DatabaseServer {
    pub fn load_playlist(self: &Arc<Self>, id: PlaylistId) -> BResult<Option<PlaylistModel>> {
        let db = self.db().begin_read()?;
        self.load_playlist_impl(&db, id)
    }

    fn load_playlist_impl(
        self: &Arc<Self>,
        db: &ReadTransaction,
        id: PlaylistId,
    ) -> BResult<Option<PlaylistModel>> {
        let table = db.open_table(TABLE_PLAYLIST)?;
        let p = table.get(id)?.map(|v| v.value());
        Ok(p)
    }

    pub fn load_playlists(self: &Arc<Self>) -> BResult<Vec<PlaylistModel>> {
        let db = self.db().begin_read()?;
        let table = db.open_table(TABLE_PLAYLIST)?;
        let len = table.len()? as usize;

        let mut ret: Vec<PlaylistModel> = Default::default();
        ret.reserve(len);

        let mut iter = table.iter()?;
        while let Some(v) = iter.next() {
            let v = v?.1.value();
            ret.push(v);
        }

        Ok(ret)
    }

    pub fn load_playlist_first_cover_id(
        self: &Arc<Self>,
        id: PlaylistId,
    ) -> BResult<Option<MusicId>> {
        let db = self.db().begin_read()?;
        let table = db.open_multimap_table(TABLE_PLAYLIST_MUSIC)?;
        let mut iter = table.get(id)?;

        while let Some(id) = iter.next() {
            let id = id?.value();
            let m = self.load_music_impl(&db, id)?.unwrap();

            if m.cover.is_some() {
                return Ok(Some(m.id));
            }
        }
        Ok(None)
    }

    pub fn create_playlist(
        self: &Arc<Self>,
        title: String,
        picture: Option<StorageEntryLoc>,
        musics: Vec<ArgDBAddMusic>,
        current_time_ms: i64,
    ) -> BResult<(PlaylistId, Vec<MusicId>)> {
        let db = self.db().begin_write()?;
        let rdb = self.db().begin_read()?;

        let mut ret: Vec<MusicId> = Default::default();
        ret.reserve(musics.len());

        let playlist_id = {
            let id = {
                let id = self.alloc_id(&db, DbKeyAlloc::Playlist)?;
                let id = PlaylistId::wrap(id);
                id
            };

            let mut playlist = PlaylistModel {
                id,
                title: Default::default(),
                created_time: Default::default(),
                picture: Default::default(),
            };

            playlist.title = title;
            playlist.picture = picture;
            playlist.created_time = current_time_ms;

            let mut table = db.open_table(TABLE_PLAYLIST)?;
            table.insert(id, playlist)?;

            id
        };
        for m in musics {
            let id = self.add_music_impl(&db, &rdb, m)?;

            let mut table_pm = db.open_multimap_table(TABLE_PLAYLIST_MUSIC)?;
            let mut table_mp = db.open_multimap_table(TABLE_MUSIC_PLAYLIST)?;
            table_pm.insert(playlist_id, id)?;
            table_mp.insert(id, playlist_id)?;

            ret.push(id);
        }

        db.commit()?;
        Ok((playlist_id, ret))
    }

    pub fn update_playlist(
        self: &Arc<Self>,
        id: PlaylistId,
        title: String,
        picture: Option<StorageEntryLoc>,
    ) -> BResult<PlaylistId> {
        let db = self.db().begin_write()?;
        let rdb = self.db().begin_read()?;

        {
            let mut playlist = self.load_playlist_impl(&rdb, id)?.unwrap();

            playlist.title = title;
            playlist.picture = picture;

            let mut table = db.open_table(TABLE_PLAYLIST)?;
            table.insert(id, playlist)?;
        };
        db.commit()?;
        Ok(id)
    }

    pub fn remove_playlist(self: &Arc<Self>, playlist_id: PlaylistId) -> BResult<()> {
        let db = self.db().begin_write()?;
        let rdb = self.db().begin_read()?;
        let mut to_remove_blobs: Vec<BlobId> = Default::default();

        {
            let mut table_playlist = db.open_table(TABLE_PLAYLIST)?;
            let mut table_pm = db.open_multimap_table(TABLE_PLAYLIST_MUSIC)?;
            let mut table_mp = db.open_multimap_table(TABLE_MUSIC_PLAYLIST)?;

            table_playlist.remove(playlist_id)?;

            let ids = table_pm.get(playlist_id)?;
            for id in ids {
                let id = id?.value();
                table_mp.remove(id, playlist_id)?;
                self.compact_music_impl(&db, &rdb, &mut table_mp, &mut to_remove_blobs, id)?;
            }
            table_pm.remove_all(playlist_id)?;
        }

        db.commit()?;

        for id in to_remove_blobs {
            self.blob().remove(id)?;
        }

        Ok(())
    }

    pub fn remove_music_from_playlist(
        self: &Arc<Self>,
        playlist_id: PlaylistId,
        music_id: MusicId,
    ) -> BResult<()> {
        let mut to_remove_blobs: Vec<BlobId> = Default::default();

        let db = self.db().begin_write()?;
        let rdb = self.db().begin_read()?;

        {
            let mut table_pm = db.open_multimap_table(TABLE_PLAYLIST_MUSIC)?;
            let mut table_mp = db.open_multimap_table(TABLE_MUSIC_PLAYLIST)?;
            table_pm.remove(playlist_id, music_id)?;
            table_mp.remove(music_id, playlist_id)?;

            self.compact_music_impl(&db, &rdb, &mut table_mp, &mut to_remove_blobs, music_id)?;
        }

        db.commit()?;

        for id in to_remove_blobs {
            self.blob().remove(id)?;
        }

        Ok(())
    }

    pub fn add_musics_to_playlist(
        self: &Arc<Self>,
        playlist_id: PlaylistId,
        musics: Vec<ArgDBAddMusic>,
    ) -> BResult<Vec<MusicId>> {
        let db = self.db().begin_write()?;
        let rdb = self.db().begin_read()?;

        let mut ret: Vec<MusicId> = Default::default();
        ret.reserve(musics.len());

        for m in musics {
            let id = self.add_music_impl(&db, &rdb, m)?;

            let mut table_pm = db.open_multimap_table(TABLE_PLAYLIST_MUSIC)?;
            let mut table_mp = db.open_multimap_table(TABLE_MUSIC_PLAYLIST)?;
            table_pm.insert(playlist_id, id)?;
            table_mp.insert(id, playlist_id)?;

            ret.push(id);
        }
        db.commit()?;
        Ok(ret)
    }
}
