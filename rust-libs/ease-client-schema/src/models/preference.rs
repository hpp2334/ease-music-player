use serde::{Deserialize, Serialize};

use crate::shared::{PlayMode, StorageEntryLoc};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PreferenceModel {
    pub playmode: PlayMode,
    /// BCP-47 tag of the in-app language override; `None` = system.
    pub language: Option<String>,
    /// Folder (storage + path) the user last imported from; `None` =
    /// never imported. Parsed/serialized at the repository boundary.
    pub last_import_loc: Option<StorageEntryLoc>,
}
