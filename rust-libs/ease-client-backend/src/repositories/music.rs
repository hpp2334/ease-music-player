use std::{sync::Arc, time::Duration};

use ease_client_migration::converter;
use ease_client_schema::entities::{music, playlist_music};
use ease_client_schema::{BlobId, MusicId, MusicModel, PlaylistId, StorageEntryLoc};
use ease_order_key::OrderKey;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};

use crate::error::BResult;

use super::core::DatabaseServer;

#[derive(Debug)]
pub struct ArgDBAddMusic {
    pub loc: StorageEntryLoc,
    pub title: String,
    /// Sibling lyric detected at import time (see
    /// `services::music::detect_import_sibling_lyrics`). Stored as the
    /// music's lyric while `lyric_default` stays set — the loc is a
    /// resolution snapshot, not a user pick, so play-time probing still
    /// falls through if the file later disappears.
    pub lyric: Option<StorageEntryLoc>,
}

impl DatabaseServer {
    pub async fn load_musics_by_playlist_id(
        self: &Arc<Self>,
        playlist_id: PlaylistId,
    ) -> BResult<Vec<MusicModel>> {
        let db = self.db();
        let pid = *playlist_id.as_ref();

        let edges = playlist_music::Entity::find()
            .filter(playlist_music::Column::PlaylistId.eq(pid))
            .all(&db)
            .await?;

        let mut ret: Vec<MusicModel> = Vec::with_capacity(edges.len());
        for edge in edges {
            if let Some(row) = music::Entity::find_by_id(edge.music_id).one(&db).await? {
                ret.push(converter::music_to_model(row));
            }
        }

        ret.sort_by(|lhs, rhs| lhs.order.cmp(&rhs.order));
        Ok(ret)
    }

    pub async fn load_music(self: &Arc<Self>, id: MusicId) -> BResult<Option<MusicModel>> {
        let db = self.db();
        let row = music::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        Ok(row.map(converter::music_to_model))
    }

    async fn load_music_by_loc(
        self: &Arc<Self>,
        loc: StorageEntryLoc,
    ) -> BResult<Option<MusicModel>> {
        let db = self.db();
        let row = music::Entity::find()
            .filter(music::Column::LocStorageId.eq(*loc.storage_id.as_ref()))
            .filter(music::Column::LocPath.eq(loc.path.clone()))
            .one(&db)
            .await?;
        Ok(row.map(converter::music_to_model))
    }

    pub async fn add_music_impl(
        self: &Arc<Self>,
        arg: ArgDBAddMusic,
        order: OrderKey,
    ) -> BResult<(MusicId, bool)> {
        if let Some(existing) = self.load_music_by_loc(arg.loc.clone()).await? {
            // Heal: attach a sibling lyric detected by this import to an
            // existing row that never had one resolved. Auto-detect only —
            // a user pick or an explicit removal cleared `lyric_default`,
            // and that decision must survive re-imports.
            if existing.lyric.is_none() && existing.lyric_default {
                if let Some(lyric) = arg.lyric {
                    self.attach_detected_music_lyric(existing.id, lyric)
                        .await?;
                }
            }
            return Ok((existing.id, true));
        }

        let db = self.db();
        let am = music::ActiveModel {
            id: ActiveValue::NotSet,
            loc_storage_id: ActiveValue::Set(*arg.loc.storage_id.as_ref()),
            loc_path: ActiveValue::Set(arg.loc.path.clone()),
            title: ActiveValue::Set(arg.title),
            artist: ActiveValue::Set(String::new()),
            duration_ms: ActiveValue::Set(None),
            cover_blob_id: ActiveValue::Set(None),
            lyric_storage_id: ActiveValue::Set(arg.lyric.as_ref().map(|l| *l.storage_id.as_ref())),
            lyric_path: ActiveValue::Set(arg.lyric.map(|l| l.path)),
            lyric_default: ActiveValue::Set(1),
            order: ActiveValue::Set(serde_json::to_string(&order.into_raw())?),
        };
        let inserted = am.insert(&db).await?;
        Ok((MusicId::wrap(inserted.id), false))
    }

    /// Persist an import-detected sibling lyric, keeping `lyric_default`
    /// set (contrast [`Self::update_music_lyric`], which records a user's
    /// explicit decision and clears the flag).
    async fn attach_detected_music_lyric(
        self: &Arc<Self>,
        id: MusicId,
        loc: StorageEntryLoc,
    ) -> BResult<()> {
        let db = self.db();
        let row = music::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: music::ActiveModel = row.into();
            am.lyric_storage_id = ActiveValue::Set(Some(*loc.storage_id.as_ref()));
            am.lyric_path = ActiveValue::Set(Some(loc.path));
            am.lyric_default = ActiveValue::Set(1);
            am.update(&db).await?;
        }
        Ok(())
    }

    pub async fn update_music_total_duration(
        self: &Arc<Self>,
        id: MusicId,
        duration: Duration,
    ) -> BResult<()> {
        let db = self.db();
        let row = music::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: music::ActiveModel = row.into();
            am.duration_ms = ActiveValue::Set(Some(duration.as_millis() as i64));
            am.update(&db).await?;
        }
        Ok(())
    }

    pub async fn update_music_artist(self: &Arc<Self>, id: MusicId, artist: String) -> BResult<()> {
        let db = self.db();
        let row = music::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: music::ActiveModel = row.into();
            am.artist = ActiveValue::Set(artist);
            am.update(&db).await?;
        }
        Ok(())
    }

    pub async fn update_music_cover(self: &Arc<Self>, id: MusicId, cover: Vec<u8>) -> BResult<()> {
        let db = self.db();
        let row = music::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let existing_cover = row.cover_blob_id;
            let mut am: music::ActiveModel = row.into();

            if let Some(old) = existing_cover {
                self.blob().remove(BlobId::wrap(old))?;
            }
            let cover_id = self.blob().write(cover).await?;
            am.cover_blob_id = ActiveValue::Set(Some(*cover_id.as_ref()));
            am.update(&db).await?;
        }
        Ok(())
    }

    pub async fn update_music_lyric(
        self: &Arc<Self>,
        id: MusicId,
        loc: Option<StorageEntryLoc>,
    ) -> BResult<()> {
        let db = self.db();
        let row = music::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: music::ActiveModel = row.into();
            am.lyric_storage_id = ActiveValue::Set(loc.as_ref().map(|l| *l.storage_id.as_ref()));
            am.lyric_path = ActiveValue::Set(loc.map(|l| l.path));
            am.lyric_default = ActiveValue::Set(0);
            am.update(&db).await?;
        }
        Ok(())
    }

    pub async fn set_music_order(self: &Arc<Self>, id: MusicId, order: OrderKey) -> BResult<()> {
        let db = self.db();
        let row = music::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: music::ActiveModel = row.into();
            am.order = ActiveValue::Set(serde_json::to_string(&order.into_raw())?);
            am.update(&db).await?;
        }
        Ok(())
    }

    /// Remove the music if it has no remaining playlist references. Returns
    /// the cover blob id to remove (if any).
    pub(crate) async fn compact_music(self: &Arc<Self>, id: MusicId) -> BResult<Option<BlobId>> {
        let db = self.db();
        let mid = *id.as_ref();
        let ref_count = playlist_music::Entity::find()
            .filter(playlist_music::Column::MusicId.eq(mid))
            .count(&db)
            .await?;

        if ref_count == 0 {
            if let Some(row) = music::Entity::find_by_id(mid).one(&db).await? {
                let cover = row.cover_blob_id;
                music::Entity::delete_by_id(mid).exec(&db).await?;
                return Ok(cover.map(BlobId::wrap));
            }
        }
        Ok(None)
    }

    #[allow(dead_code)]
    pub async fn list_music_ordered(self: &Arc<Self>) -> BResult<Vec<MusicModel>> {
        let db = self.db();
        let rows = music::Entity::find()
            .order_by_asc(music::Column::Id)
            .all(&db)
            .await?;
        Ok(rows.into_iter().map(converter::music_to_model).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ease_client_schema::StorageId;

    async fn server() -> (std::sync::Arc<DatabaseServer>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let server = DatabaseServer::new();
        server
            .init(dir.path().to_str().unwrap().to_string())
            .await
            .unwrap();
        (server, dir)
    }

    fn add_arg(path: &str, lyric: Option<StorageEntryLoc>) -> ArgDBAddMusic {
        ArgDBAddMusic {
            loc: StorageEntryLoc {
                storage_id: StorageId::wrap(1),
                path: path.to_string(),
            },
            title: path.to_string(),
            lyric,
        }
    }

    fn lyric(path: &str) -> Option<StorageEntryLoc> {
        Some(StorageEntryLoc {
            storage_id: StorageId::wrap(1),
            path: path.to_string(),
        })
    }

    #[tokio::test]
    async fn insert_persists_detected_lyric_keeping_default_flag() {
        let (server, _dir) = server().await;

        let (id, existing) = server
            .add_music_impl(add_arg("/Music/a.mp3", lyric("/Music/a.lrc")), OrderKey::default())
            .await
            .unwrap();
        assert!(!existing);
        let m = server.load_music(id).await.unwrap().unwrap();
        assert_eq!(m.lyric, lyric("/Music/a.lrc"));
        // Detected ≠ user-picked: the flag stays set so play-time probing
        // still falls through if the file later disappears.
        assert!(m.lyric_default);

        // No detected sibling → plain row, exactly like before.
        let (id, _) = server
            .add_music_impl(add_arg("/Music/b.mp3", None), OrderKey::default())
            .await
            .unwrap();
        let m = server.load_music(id).await.unwrap().unwrap();
        assert_eq!(m.lyric, None);
        assert!(m.lyric_default);
    }

    #[tokio::test]
    async fn reimport_heals_existing_row_without_lyric() {
        let (server, _dir) = server().await;

        let (id, _) = server
            .add_music_impl(add_arg("/Music/a.mp3", None), OrderKey::default())
            .await
            .unwrap();
        // The lyric appeared later (or detection is new): a re-import that
        // detects it attaches it to the existing row.
        let (id2, existing) = server
            .add_music_impl(add_arg("/Music/a.mp3", lyric("/Music/a.lrc")), OrderKey::default())
            .await
            .unwrap();
        assert!(existing);
        assert_eq!(id, id2);
        let m = server.load_music(id).await.unwrap().unwrap();
        assert_eq!(m.lyric, lyric("/Music/a.lrc"));
        assert!(m.lyric_default);
    }

    #[tokio::test]
    async fn reimport_never_clobbers_user_decision() {
        let (server, _dir) = server().await;

        let (id, _) = server
            .add_music_impl(add_arg("/Music/a.mp3", None), OrderKey::default())
            .await
            .unwrap();

        // User picks an explicit lyric (clears `lyric_default`) ...
        server
            .update_music_lyric(id, lyric("/Music/custom.lrc"))
            .await
            .unwrap();
        // ... a later re-import must not overwrite it.
        server
            .add_music_impl(add_arg("/Music/a.mp3", lyric("/Music/a.lrc")), OrderKey::default())
            .await
            .unwrap();
        let m = server.load_music(id).await.unwrap().unwrap();
        assert_eq!(m.lyric, lyric("/Music/custom.lrc"));
        assert!(!m.lyric_default);

        // Same for an explicit removal: `lyric == None` with the flag
        // cleared stays untouched.
        server.update_music_lyric(id, None).await.unwrap();
        server
            .add_music_impl(add_arg("/Music/a.mp3", lyric("/Music/a.lrc")), OrderKey::default())
            .await
            .unwrap();
        let m = server.load_music(id).await.unwrap().unwrap();
        assert_eq!(m.lyric, None);
        assert!(!m.lyric_default);
    }
}
