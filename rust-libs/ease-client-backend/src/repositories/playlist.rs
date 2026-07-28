use std::sync::Arc;

use ease_client_migration::converter;
use ease_client_schema::entities::{music, playlist, playlist_music};
use ease_client_schema::{BlobId, MusicId, PlaylistId, PlaylistModel, StorageEntryLoc};
use ease_order_key::OrderKey;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use crate::error::BResult;

use super::{core::DatabaseServer, music::ArgDBAddMusic};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedMusic {
    pub id: MusicId,
    pub existed: bool,
}

impl DatabaseServer {
    pub async fn load_playlist(self: &Arc<Self>, id: PlaylistId) -> BResult<Option<PlaylistModel>> {
        let db = self.db();
        let row = playlist::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        Ok(row.map(converter::playlist_to_model))
    }

    pub async fn load_playlists(self: &Arc<Self>) -> BResult<Vec<PlaylistModel>> {
        let db = self.db();
        let rows = playlist::Entity::find().all(&db).await?;
        let mut ret: Vec<PlaylistModel> = rows
            .into_iter()
            .map(converter::playlist_to_model)
            .collect();
        ret.sort_by_key(|v| OrderKey::wrap(v.order.clone()));
        Ok(ret)
    }

    pub async fn create_playlist(
        self: &Arc<Self>,
        title: String,
        picture: Option<StorageEntryLoc>,
        musics: Vec<ArgDBAddMusic>,
        current_time_ms: i64,
        order: OrderKey,
    ) -> BResult<(PlaylistId, Vec<AddedMusic>)> {
        let db = self.db();

        let playlist_am = playlist::ActiveModel {
            id: ActiveValue::NotSet,
            title: ActiveValue::Set(title),
            created_time: ActiveValue::Set(current_time_ms),
            picture_storage_id: ActiveValue::Set(picture.as_ref().map(|p| *p.storage_id.as_ref())),
            picture_path: ActiveValue::Set(picture.map(|p| p.path)),
            order: ActiveValue::Set(serde_json::to_string(&order.into_raw())?),
        };
        let inserted = playlist_am.insert(&db).await?;
        let playlist_id = PlaylistId::wrap(inserted.id);

        let mut ret: Vec<AddedMusic> = Vec::with_capacity(musics.len());
        let mut order = OrderKey::default();
        for m in musics {
            let (mid, existed) = self.add_music_impl(m, order.clone()).await?;
            order = OrderKey::greater(&order);

            // Same duplicate-guard as in add_musics_to_playlist —
            // handles a user passing the same path twice in one batch.
            let already_linked = playlist_music::Entity::find()
                .filter(playlist_music::Column::PlaylistId.eq(*playlist_id.as_ref()))
                .filter(playlist_music::Column::MusicId.eq(*mid.as_ref()))
                .one(&db)
                .await?
                .is_some();

            if !already_linked {
                playlist_music::ActiveModel {
                    playlist_id: ActiveValue::Set(*playlist_id.as_ref()),
                    music_id: ActiveValue::Set(*mid.as_ref()),
                }
                .insert(&db)
                .await?;
            }

            ret.push(AddedMusic {
                id: mid,
                existed: existed || already_linked,
            });
        }

        Ok((playlist_id, ret))
    }

    pub async fn update_playlist(
        self: &Arc<Self>,
        id: PlaylistId,
        title: String,
        picture: Option<StorageEntryLoc>,
    ) -> BResult<PlaylistId> {
        let db = self.db();
        let row = playlist::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: playlist::ActiveModel = row.into();
            am.title = ActiveValue::Set(title);
            am.picture_storage_id = ActiveValue::Set(picture.as_ref().map(|p| *p.storage_id.as_ref()));
            am.picture_path = ActiveValue::Set(picture.map(|p| p.path));
            am.update(&db).await?;
        }
        Ok(id)
    }

    pub async fn set_playlist_order(
        self: &Arc<Self>,
        id: PlaylistId,
        order: OrderKey,
    ) -> BResult<()> {
        let db = self.db();
        let row = playlist::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        if let Some(row) = row {
            let mut am: playlist::ActiveModel = row.into();
            am.order = ActiveValue::Set(serde_json::to_string(&order.into_raw())?);
            am.update(&db).await?;
        }
        Ok(())
    }

    pub async fn remove_playlist(self: &Arc<Self>, playlist_id: PlaylistId) -> BResult<()> {
        let db = self.db();
        let pid = *playlist_id.as_ref();

        // Find all music in this playlist, then detach.
        let edges = playlist_music::Entity::find()
            .filter(playlist_music::Column::PlaylistId.eq(pid))
            .all(&db)
            .await?;

        playlist_music::Entity::delete_many()
            .filter(playlist_music::Column::PlaylistId.eq(pid))
            .exec(&db)
            .await?;

        playlist::Entity::delete_by_id(pid).exec(&db).await?;

        let mut to_remove_blobs: Vec<BlobId> = Default::default();
        for edge in edges {
            if let Some(blob) = self.compact_music(MusicId::wrap(edge.music_id)).await? {
                to_remove_blobs.push(blob);
            }
        }

        for blob_id in to_remove_blobs {
            self.blob().remove(blob_id)?;
        }
        Ok(())
    }

    pub async fn remove_music_from_playlist(
        self: &Arc<Self>,
        playlist_id: PlaylistId,
        music_id: MusicId,
    ) -> BResult<()> {
        let db = self.db();
        playlist_music::Entity::delete_many()
            .filter(playlist_music::Column::PlaylistId.eq(*playlist_id.as_ref()))
            .filter(playlist_music::Column::MusicId.eq(*music_id.as_ref()))
            .exec(&db)
            .await?;

        if let Some(blob) = self.compact_music(music_id).await? {
            self.blob().remove(blob)?;
        }
        Ok(())
    }

    pub async fn add_musics_to_playlist(
        self: &Arc<Self>,
        playlist_id: PlaylistId,
        musics: Vec<ArgDBAddMusic>,
        last_order: OrderKey,
    ) -> BResult<Vec<AddedMusic>> {
        let db = self.db();
        let mut order = OrderKey::greater(&last_order);
        let mut ret: Vec<AddedMusic> = Vec::with_capacity(musics.len());

        for m in musics {
            let (mid, existed) = self.add_music_impl(m, order.clone()).await?;
            order = OrderKey::greater(&order);

            // Skip linking if the music is already in this playlist
            // (e.g. user re-imported an existing entry). Without this
            // guard the INSERT below violates the UNIQUE(playlist_id,
            // music_id) constraint and aborts the entire batch —
            // partial successes get committed but the call returns an
            // error envelope, so the Kotlin side can't tell which
            // rows were actually inserted (and skips duration probes).
            let already_linked = playlist_music::Entity::find()
                .filter(playlist_music::Column::PlaylistId.eq(*playlist_id.as_ref()))
                .filter(playlist_music::Column::MusicId.eq(*mid.as_ref()))
                .one(&db)
                .await?
                .is_some();

            if !already_linked {
                playlist_music::ActiveModel {
                    playlist_id: ActiveValue::Set(*playlist_id.as_ref()),
                    music_id: ActiveValue::Set(*mid.as_ref()),
                }
                .insert(&db)
                .await?;
            }

            ret.push(AddedMusic {
                id: mid,
                existed: existed || already_linked,
            });
        }
        Ok(ret)
    }
}

#[allow(dead_code)]
fn _silence_unused() {
    let _ = music::Entity;
}
