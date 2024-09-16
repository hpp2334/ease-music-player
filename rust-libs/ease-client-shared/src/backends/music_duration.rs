use std::{ops::Deref, time::Duration};

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
