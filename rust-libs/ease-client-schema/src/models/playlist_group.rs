use serde::{Deserialize, Serialize};

use crate::shared::PlaylistGroupId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistGroupModel {
    pub id: PlaylistGroupId,
    pub title: String,
    pub created_time: i64,
    /// ease-order-key raw — the group ladder, independent of the
    /// playlists' own global order ladder.
    pub order: Vec<u32>,
    /// Persisted UI expand/collapse state.
    pub expanded: bool,
}
