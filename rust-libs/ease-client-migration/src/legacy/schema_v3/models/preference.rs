use serde::{Deserialize, Serialize};

use super::super::objects::PlayMode;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
}
