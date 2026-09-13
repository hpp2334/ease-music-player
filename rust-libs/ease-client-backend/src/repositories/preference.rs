use std::sync::Arc;

use ease_client_migration::converter;
use ease_client_schema::entities::preference;
use ease_client_schema::{PlayMode, PreferenceModel, StorageId};
use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};

use crate::error::BResult;

use super::core::DatabaseServer;

fn playmode_to_i32(p: PlayMode) -> i32 {
    match p {
        PlayMode::Single => 0,
        PlayMode::SingleLoop => 1,
        PlayMode::List => 2,
        PlayMode::ListLoop => 3,
    }
}

impl DatabaseServer {
    pub async fn load_preference(self: &Arc<Self>) -> BResult<PreferenceModel> {
        let db = self.db();
        let v = preference::Entity::find_by_id(0)
            .one(&db)
            .await?
            .map(converter::preference_to_model)
            .unwrap_or_default();
        Ok(v)
    }

    pub async fn save_preference(self: &Arc<Self>, model: PreferenceModel) -> BResult<()> {
        let db = self.db();
        let existing = preference::Entity::find_by_id(0).one(&db).await?;
        let pm = playmode_to_i32(model.playmode);
        let last_import_loc =
            converter::preference_loc_to_json(model.last_import_loc);
        match existing {
            Some(row) => {
                let mut am: preference::ActiveModel = row.into();
                am.playmode = ActiveValue::Set(pm);
                am.language = ActiveValue::Set(model.language);
                am.last_import_loc = ActiveValue::Set(last_import_loc);
                am.update(&db).await?;
            }
            None => {
                let am = preference::ActiveModel {
                    id: ActiveValue::Set(0),
                    playmode: ActiveValue::Set(pm),
                    language: ActiveValue::Set(model.language),
                    last_import_loc: ActiveValue::Set(last_import_loc),
                };
                am.insert(&db).await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ease_client_schema::StorageEntryLoc;

    async fn server() -> (std::sync::Arc<DatabaseServer>, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let server = DatabaseServer::new();
        server
            .init(dir.path().to_str().unwrap().to_string())
            .await
            .unwrap();
        (server, dir)
    }

    #[tokio::test]
    async fn last_import_loc_round_trips_and_defaults_to_none() {
        let (server, _dir) = server().await;

        // Fresh database: nothing imported yet.
        assert!(server.load_preference().await.unwrap().last_import_loc.is_none());

        // First save goes through the insert branch.
        let loc = StorageEntryLoc {
            storage_id: StorageId::wrap(3),
            path: "/Music/Album".to_string(),
        };
        let mut m = server.load_preference().await.unwrap();
        m.last_import_loc = Some(loc.clone());
        server.save_preference(m).await.unwrap();
        assert_eq!(
            server.load_preference().await.unwrap().last_import_loc,
            Some(loc)
        );

        // A later save goes through the update branch and keeps the
        // other preference fields intact.
        let next = StorageEntryLoc {
            storage_id: StorageId::wrap(4),
            path: "/".to_string(),
        };
        let mut m = server.load_preference().await.unwrap();
        m.playmode = PlayMode::SingleLoop;
        m.language = Some("zh-CN".to_string());
        m.last_import_loc = Some(next.clone());
        server.save_preference(m).await.unwrap();
        let m = server.load_preference().await.unwrap();
        assert_eq!(m.playmode, PlayMode::SingleLoop);
        assert_eq!(m.language.as_deref(), Some("zh-CN"));
        assert_eq!(m.last_import_loc, Some(next));
    }

    #[tokio::test]
    async fn malformed_last_import_loc_column_degrades_to_none() {
        let (server, _dir) = server().await;

        // Write garbage straight into the column of an existing row
        // (simulating a corrupt or hand-edited database); the read must
        // degrade to `None`.
        server
            .save_preference(PreferenceModel::default())
            .await
            .unwrap();
        let mut am: preference::ActiveModel = preference::Entity::find_by_id(0)
            .one(&server.db())
            .await
            .unwrap()
            .unwrap()
            .into();
        am.last_import_loc = ActiveValue::Set(Some("{not json".to_string()));
        am.update(&server.db()).await.unwrap();

        assert!(server.load_preference().await.unwrap().last_import_loc.is_none());
    }
}
