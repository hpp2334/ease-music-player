pub mod app;
pub mod lyric;

#[macro_use]
pub mod message;
pub mod generated;

pub mod connector;
pub mod music;
pub mod music_duration;
pub mod player;
pub mod playlist;
pub mod storage;

mod env;

pub use message::{decode_message_payload, encode_message_payload, IMessage, MessagePayload};
