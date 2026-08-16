use std::sync::Arc;

use ease_client_schema::entities::{music, playlist_music, storage};
use ease_client_schema::{BlobId, StorageHandle, StorageId};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};

use crate::error::BResult;

use super::core::DatabaseServer;

impl DatabaseServer {
    pub async fn load_storage_music_count(&self, id: StorageId) -> BResult<u64> {
        let db = self.db();
        let count = music::Entity::find()
            .filter(music::Column::LocStorageId.eq(*id.as_ref()))
            .count(&db)
            .await?;
        Ok(count as u64)
    }

    /// Load one registry row.
    pub async fn load_storage_row(&self, id: StorageId) -> BResult<Option<storage::Model>> {
        Ok(storage::Entity::find_by_id(*id.as_ref())
            .one(&self.db())
            .await?)
    }

    /// Load every registry row.
    pub async fn load_all_storage_rows(&self) -> BResult<Vec<storage::Model>> {
        Ok(storage::Entity::find().all(&self.db()).await?)
    }

    /// Find-or-create the registry row for a handle. Idempotent — the
    /// uniqueness index (`idx_storage_plugin`) makes the find authoritative
    /// for Plugin.
    pub async fn obtain_storage(&self, handle: &StorageHandle) -> BResult<StorageId> {
        let db = self.db();
        let typ_i = handle.storage_type().as_i32();

        let existing = match handle {
            StorageHandle::Local => {
                storage::Entity::find()
                    .filter(storage::Column::Type.eq(typ_i))
                    .one(&db)
                    .await?
            }
            StorageHandle::Plugin {
                plugin_id,
                plugin_storage_id,
            } => {
                storage::Entity::find()
                    .filter(storage::Column::Type.eq(typ_i))
                    .filter(storage::Column::PluginId.eq(&plugin_id.id))
                    .filter(storage::Column::PluginStorageId.eq(&plugin_storage_id.id))
                    .one(&db)
                    .await?
            }
        };
        if let Some(row) = existing {
            return Ok(StorageId::wrap(row.id));
        }

        let am = storage::ActiveModel {
            id: ActiveValue::NotSet,
            r#type: ActiveValue::Set(typ_i),
            plugin_id: ActiveValue::Set(match handle {
                StorageHandle::Plugin { plugin_id, .. } => Some(plugin_id.id.clone()),
                _ => None,
            }),
            plugin_storage_id: ActiveValue::Set(match handle {
                StorageHandle::Plugin {
                    plugin_storage_id, ..
                } => Some(plugin_storage_id.id.clone()),
                _ => None,
            }),
        };
        let m = am.insert(&db).await?;
        Ok(StorageId::wrap(m.id))
    }

    /// Remove a registry row and cascade: detach + delete every music whose
    /// `loc_storage_id` points here (and its cover blob), then drop the row.
    /// Plugin detail (kv + secret) is the plugin's responsibility.
    pub async fn remove_storage(self: &Arc<Self>, id: StorageId) -> BResult<()> {
        let db = self.db();
        let reg = storage::Entity::find_by_id(*id.as_ref())
            .one(&db)
            .await?;
        if reg.is_none() {
            return Ok(());
        }

        let musics = music::Entity::find()
            .filter(music::Column::LocStorageId.eq(*id.as_ref()))
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

        storage::Entity::delete_by_id(*id.as_ref()).exec(&db).await?;

        for blob_id in to_remove_blobs {
            self.blob().remove(blob_id)?;
        }
        Ok(())
    }
}
