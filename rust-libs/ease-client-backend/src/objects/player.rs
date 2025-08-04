#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, bitcode::Encode, bitcode::Decode, uniffi::Enum,
)]
pub enum PlayMode {
    #[default]
    Single,
    SingleLoop,
    List,
    ListLoop,
}
