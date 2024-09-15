use misty_vm::MistyState;
use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, FromPrimitive, ToPrimitive)]
pub enum PreferenceDataKey {
    PlayMode,
}

#[derive(Debug, Default, Clone, MistyState, Serialize, Deserialize)]
pub struct PreferenceState {
    pub play_mode: PlayMode,
}
