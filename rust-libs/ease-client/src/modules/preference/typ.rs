use ease_client_shared::{backends::preference::PreferenceData, uis::preference::PlayMode};
use misty_vm::MistyState;
use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, MistyState, Serialize, Deserialize)]
pub struct PreferenceState {
    pub data: PreferenceData,
}
