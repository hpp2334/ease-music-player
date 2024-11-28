use std::time::Duration;

use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

use super::{
    music::{MusicAbstract, MusicId},
    playlist::PlaylistId,
    storage::DataSourceKey,
};

#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct ArgPlayMusic {
    pub id: MusicId,
    pub playlist_id: PlaylistId,
}

#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct PlayerCurrentPlaying {
    pub abstr: MusicAbstract,
    pub playlist_id: PlaylistId,
    pub index: usize,
    pub mode: PlayMode,
    pub can_prev: bool,
    pub can_next: bool,
    pub cover: Option<DataSourceKey>,
    pub prev_cover: Option<DataSourceKey>,
    pub next_cover: Option<DataSourceKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode, uniffi::Enum)]
pub enum PlayerDelegateEvent {
    Complete,
    Loading,
    Loaded,
    Play,
    Pause,
    Stop,
    Seek,
    Total {
        id: MusicId,
        duration_ms: u64,
    },
    Cover {
        id: MusicId,
        #[serde(with = "serde_bytes")]
        buffer: Vec<u8>,
    },
    Error {
        msg: String,
    },
}

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    FromPrimitive,
    ToPrimitive,
    bitcode::Encode,
    bitcode::Decode,
    uniffi::Enum,
)]
pub enum PlayMode {
    #[default]
    Single,
    SingleLoop,
    List,
    ListLoop,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ConnectorPlayerAction {
    Playing { value: bool },
    Seeked,
    Current { value: Option<PlayerCurrentPlaying> },
    Playmode { value: PlayMode },
    Loaded,
    Loading,
    Error { value: String },
}

#[derive(Debug, Serialize, Deserialize, bitcode::Encode, bitcode::Decode, uniffi::Record)]
pub struct PlayerDurations {
    pub current: Duration,
    pub buffer: Duration,
}
