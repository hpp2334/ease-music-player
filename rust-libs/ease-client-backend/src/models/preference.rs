use crate::PlayMode;

#[derive(Debug, Default, Clone, bitcode::Encode, bitcode::Decode)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
}
