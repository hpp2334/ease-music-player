use serde::{Deserialize, Serialize};

use crate::{define_message, uis::preference::PlayMode};

use super::code::Code;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceData {
    pub playmode: PlayMode,
}

define_message!(GetPreferenceMsg, Code::GetPreference, (), PreferenceData);

define_message!(
    UpdatePreferencePlaymodeMsg,
    Code::UpdatePreferencePlaymode,
    PlayMode,
    ()
);
