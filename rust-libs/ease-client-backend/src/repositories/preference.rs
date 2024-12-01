use ease_client_shared::backends::player::PlayMode;

#[derive(Debug, Default, Clone, bitcode::Encode, bitcode::Decode)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
}
