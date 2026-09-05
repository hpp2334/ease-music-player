use serde::{Deserialize, Serialize};

use crate::shared::PlayMode;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
    /// BCP-47 tag of the in-app language override; `None` = system.
    pub language: Option<String>,
}
