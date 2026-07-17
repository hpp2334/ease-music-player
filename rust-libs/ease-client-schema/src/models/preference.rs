use serde::{Deserialize, Serialize};

use crate::shared::PlayMode;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
}
