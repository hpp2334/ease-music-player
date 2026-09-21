use serde::{Deserialize, Serialize};

use crate::shared::{PlayMode, StorageEntryLoc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
    /// BCP-47 tag of the in-app language override; `None` = system.
    pub language: Option<String>,
    /// Folder (storage + path) the user last imported from; `None` =
    /// never imported. Parsed/serialized at the repository boundary.
    pub last_import_loc: Option<StorageEntryLoc>,
    /// Whether track artist lines are shown in the UI. Defaults to
    /// `true` — both the DB column and the in-memory default are on, so
    /// a not-yet-persisted preference still shows artists.
    pub show_track_artist: bool,
}

impl Default for PreferenceModel {
    fn default() -> Self {
        Self {
            playmode: PlayMode::default(),
            language: None,
            last_import_loc: None,
            show_track_artist: true,
        }
    }
}
