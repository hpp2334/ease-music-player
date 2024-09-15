use serde::{Deserialize, Serialize};

use crate::models::music::{MusicDuration, MusicId, MusicModel};

#[derive(Debug, Serialize, Deserialize)]
pub struct MusicMeta {
    pub id: MusicId,
    pub title: String,
    pub duration: Option<MusicDuration>,
}

pub(crate) fn build_music_meta(model: MusicModel) -> MusicMeta {
    MusicMeta {
        id: model.id,
        title: model.title,
        duration: model.duration,
    }
}
