use serde::Serialize;

use crate::backends::{music::MusicId, playlist::PlaylistId};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, uniffi::Enum)]
pub enum CreatePlaylistMode {
    #[default]
    Full,
    Empty,
}
