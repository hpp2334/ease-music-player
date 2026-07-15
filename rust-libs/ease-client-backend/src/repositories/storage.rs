use std::sync::Arc;

use ease_client_migration::converter;
use ease_client_migration::entities::{music, playlist_music, storage};
use ease_client_schema::{BlobId, StorageId, StorageModel};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

use crate::{error::BResult, objects::ArgUpsertStorage};

use super::core::DatabaseServer;

impl DatabaseServer {
    pub async fn load_storage_music_count(self: &Arc<Self>, id: StorageId) -> BResult<u64> {
        let db = self.db();
        let count = music::Entity::find()
            .filter(music::Column::LocStorageId.eq(*id.as_ref()))
            .count(&db)
            .await?;
        Ok(count as u64)
    }

    pub async fn load_storage(self: &Arc<Self>, id: StorageId) -> BResult<Option<StorageModel>> {
        let db = self.db();
        let row = storage::Entity::find_by_id(*id.as_ref()).one(&db).await?;
        Ok(row.map(converter::storage_to_model))
    }

    pub async fn load_storages(self: &Arc<Self>) -> BResult<Vec<StorageModel>> {
        let db = self.db();
        let rows = storage::Entity::find().all(&db).await?;
        Ok(rows.into_iter().map(converter::storage_to_model).collect())
    }

    pub async fn upsert_storage(self: &Arc<Self>, arg: ArgUpsertStorage) -> BResult<StorageId> {
        let db = self.db();
        let typ_i = match arg.typ {
            ease_client_schema::StorageType::Local => 0,
            ease_client_schema::StorageType::Webdav => 1,
            ease_client_schema::StorageType::OneDrive => 2,
        };
        let id = match arg.id {
            Some(id) => {
                let am = storage::ActiveModel {
                    id: ActiveValue::Unchanged(*id.as_ref()),
                    addr: ActiveValue::Set(arg.addr),
                    alias: ActiveValue::Set(arg.alias),
                    username: ActiveValue::Set(arg.username),
                    password: ActiveValue::Set(arg.password),
                    is_anonymous: ActiveValue::Set(if arg.is_anonymous { 1 } else { 0 }),
                    typ: ActiveValue::Set(typ_i),
                };
                let updated = am.update(&db).await?;
                StorageId::wrap(updated.id)
            }
            None => {
                let am = storage::ActiveModel {
                    id: ActiveValue::NotSet,
                    addr: ActiveValue::Set(arg.addr),
                    alias: ActiveValue::Set(arg.alias),
                    username: ActiveValue::Set(arg.username),
                    password: ActiveValue::Set(arg.password),
                    is_anonymous: ActiveValue::Set(if arg.is_anonymous { 1 } else { 0 }),
                    typ: ActiveValue::Set(typ_i),
                };
                let inserted = am.insert(&db).await?;
                StorageId::wrap(inserted.id)
            }
        };

        Ok(id)
    }

    pub async fn remove_storage(self: &Arc<Self>, id: StorageId) -> BResult<()> {
        let db = self.db();
        let id_val = *id.as_ref();

        // Cascade: for each music in this storage, detach from playlists and
        // remove its cover blob.
        let musics = music::Entity::find()
            .filter(music::Column::LocStorageId.eq(id_val))
            .all(&db)
            .await?;

        let mut to_remove_blobs: Vec<BlobId> = Default::default();
        for m in musics {
            playlist_music::Entity::delete_many()
                .filter(playlist_music::Column::MusicId.eq(m.id))
                .exec(&db)
                .await?;
            if let Some(cover) = m.cover_blob_id {
                to_remove_blobs.push(BlobId::wrap(cover));
            }
            music::Entity::delete_by_id(m.id).exec(&db).await?;
        }

        storage::Entity::delete_by_id(id_val).exec(&db).await?;

        for blob_id in to_remove_blobs {
            self.blob().remove(blob_id)?;
        }

        Ok(())
    }
}
