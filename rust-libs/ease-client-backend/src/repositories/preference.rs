use std::sync::Arc;

use ease_client_migration::converter;
use ease_client_schema::entities::preference;
use ease_client_schema::{PlayMode, PreferenceModel};
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
        match existing {
            Some(row) => {
                let mut am: preference::ActiveModel = row.into();
                am.playmode = ActiveValue::Set(pm);
                am.language = ActiveValue::Set(model.language);
                am.update(&db).await?;
            }
            None => {
                let am = preference::ActiveModel {
                    id: ActiveValue::Set(0),
                    playmode: ActiveValue::Set(pm),
                    language: ActiveValue::Set(model.language),
                };
                am.insert(&db).await?;
            }
        }
        Ok(())
    }
}
