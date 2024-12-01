use std::{ops::Deref, time::Duration};

#[derive(Debug, Clone, Copy, bitcode::Encode, bitcode::Decode)]
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
