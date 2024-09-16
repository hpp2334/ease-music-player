use misty_serve::define_message;
use serde::{Deserialize, Serialize};

use crate::uis::preference::PlayMode;

use super::code::Code;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceData {
    pub play_mode: PlayMode,
}

define_message!(GetPreferenceMsg, Code::GetPreference, (), PreferenceData);
define_message!(
    UpdatePreferenceMsg,
    Code::UpdatePreference,
    PreferenceData,
    ()
);
