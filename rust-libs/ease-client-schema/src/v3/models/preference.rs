use serde::{Deserialize, Serialize};

use crate::v2::PlayMode;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
}
