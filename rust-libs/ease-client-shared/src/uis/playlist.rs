use serde::Serialize;


#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum CreatePlaylistMode {
    #[default]
    Full,
    Empty,
}
