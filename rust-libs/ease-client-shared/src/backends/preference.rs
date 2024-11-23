use serde::{Deserialize, Serialize};

use super::player::PlayMode;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceData {
    pub playmode: PlayMode,
}
