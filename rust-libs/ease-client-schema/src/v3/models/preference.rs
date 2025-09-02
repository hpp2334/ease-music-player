use serde::{Deserialize, Serialize};

use crate::PlayMode;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
}
