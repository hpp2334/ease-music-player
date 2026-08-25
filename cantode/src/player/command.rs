//! The handle→worker command protocol.
//!
//! Every public [`Player`](super::Player) method posts a [`Command`]
//! onto the worker's mpsc channel and returns immediately. The commands
//! that must report a result (`load`, `seek`, `unload`) carry a reply
//! sender the worker answers when the operation completes; the caller
//! blocks on that reply channel, not on the worker.

use std::sync::mpsc;
use std::time::Duration;

use crate::{AudioSource, CantodeError, Metadata};

/// Capacity of the command channel. Small because commands are rare
/// relative to decode work; backpressure is fine (the worker drains fast).
pub(super) const COMMAND_CHANNEL_CAP: usize = 32;

/// Internal command set posted by the public API to the worker.
pub(crate) enum Command {
    /// Load a fresh source. The worker rebuilds the decoder and primes the
    /// sink. The `SyncSender` lets the worker report the resulting
    /// [`Metadata`] (or error) back to the caller of `load`.
    Load {
        source: Box<dyn AudioSource>,
        reply: mpsc::Sender<LoadResult>,
    },
    Play,
    Pause,
    Stop,
    Seek {
        target: Duration,
        reply: mpsc::Sender<SeekResult>,
    },
    Unload {
        reply: mpsc::Sender<crate::Result<()>>,
    },
    SetVolume(f32),
    Shutdown,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Command::Load { .. } => f.debug_struct("Command::Load").finish_non_exhaustive(),
            Command::Play => write!(f, "Command::Play"),
            Command::Pause => write!(f, "Command::Pause"),
            Command::Stop => write!(f, "Command::Stop"),
            Command::Seek { target, .. } => f
                .debug_struct("Command::Seek")
                .field("target", target)
                .finish_non_exhaustive(),
            Command::Unload { .. } => f.debug_struct("Command::Unload").finish_non_exhaustive(),
            Command::SetVolume(v) => f.debug_tuple("Command::SetVolume").field(v).finish(),
            Command::Shutdown => write!(f, "Command::Shutdown"),
        }
    }
}

/// Result of a `Load` command.
pub(crate) enum LoadResult {
    Ok(Metadata),
    Err(CantodeError),
}

/// Result of a `Seek` command.
pub(crate) enum SeekResult {
    Ok(Duration),
    Err(CantodeError),
}
