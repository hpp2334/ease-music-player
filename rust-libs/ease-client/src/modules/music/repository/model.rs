use std::{ops::Deref, time::Duration};

use crate::modules::StorageId;
use getset::Getters;
use misty_vm::resources::MistyResourceHandle;
use serde::{Deserialize, Serialize};

use super::super::typ::*;

#[derive(Debug, Clone, Copy)]
pub struct MusicDuration(Duration);

impl Deref for MusicDuration {
    type Target = Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl MusicDuration {
    pub fn new(duration: Duration) -> Self {
        Self(duration)
    }
}

impl ease_database::ToSql for MusicDuration {
    fn to_sql(&self) -> ease_database::Result<ease_database::ToSqlOutput<'_>> {
        Ok(ease_database::ToSqlOutput::Owned(
            ease_database::Value::Integer(self.0.as_secs() as i64),
        ))
    }
}
impl ease_database::FromSql for MusicDuration {
    fn column_result(value: ease_database::ValueRef<'_>) -> ease_database::FromSqlResult<Self> {
        let v = value.as_i64().unwrap();
        Ok(Self(Duration::from_secs(v as u64)))
    }
}

impl serde::Serialize for MusicDuration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (self.0.as_secs() as i64).serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for MusicDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        i64::deserialize(deserializer).map(|p| MusicDuration(Duration::from_secs(p as u64)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicModel {
    pub id: MusicId,
    pub path: String,
    pub storage_id: StorageId,
    pub title: String,
    pub duration: Option<MusicDuration>,
    #[serde(with = "serde_bytes")]
    pub picture: Option<Vec<u8>>,
    pub lyric_storage_id: Option<StorageId>,
    pub lyric_path: Option<String>,
}

#[derive(Debug, Clone, Getters)]
pub struct Music {
    pub(super) model: MusicModel,
    #[getset(get = "pub")]
    pub(super) picture: Option<MistyResourceHandle>,
}

impl Music {
    pub fn id(&self) -> MusicId {
        self.model.id
    }

    pub fn entry(&self) -> (StorageId, String) {
        (self.model.storage_id, self.model.path.clone())
    }

    pub fn title(&self) -> &str {
        &self.model.title
    }

    pub fn duration(&self) -> Option<MusicDuration> {
        self.model.duration.clone()
    }

    pub fn set_duration(&mut self, duration: Option<MusicDuration>) {
        self.model.duration = duration;
    }

    pub fn lyric_entry(&self) -> Option<(StorageId, String)> {
        if self.model.lyric_storage_id.is_some() && self.model.lyric_path.is_some() {
            return Some((
                self.model.lyric_storage_id.unwrap(),
                self.model.lyric_path.clone().unwrap(),
            ));
        }
        return None;
    }

    pub fn set_lyric_entry(&mut self, value: Option<(StorageId, String)>) {
        if let Some((storage_id, path)) = value {
            self.model.lyric_storage_id = Some(storage_id);
            self.model.lyric_path = Some(path);
        } else {
            self.model.lyric_storage_id = None;
            self.model.lyric_path = None;
        }
    }
}
