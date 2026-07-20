//! # cantode — a cross-platform audio engine
//!
//! `cantode` decodes, outputs, and probes audio behind a pluggable,
//! trait-based API. It is designed as a future replacement for media3
//! ExoPlayer in [Ease Music Player][ease], but has no dependency on that
//! project — it is a standalone, runtime-agnostic library.
//!
//! ## Layered, trait-based architecture
//!
//! ```text
//!   ┌─────────────────────────────────────────────────────┐
//!   │  Player (orchestrator: state machine + worker)      │
//!   ├─────────────────────────────────────────────────────┤
//!   │  Decoder (trait)         AudioSink (trait)          │
//!   │   └─ SymphoniaDecoder      └─ CpalSink / NullSink   │
//!   ├─────────────────────────────────────────────────────┤
//!   │  AudioSource (trait: Read + Seek + Send + Sync)     │
//!   │   └─ MemoryAudioSource (tests)                      │
//!   └─────────────────────────────────────────────────────┘
//! ```
//!
//! Every layer is a trait; default implementations ship behind feature
//! flags. Embedders substitute their own `AudioSource` (HTTP range
//! reader, WebDAV client, ...), `Decoder` (platform `MediaCodec`), or
//! `AudioSink` without forking.
//!
//! ## No async runtime
//!
//! `cantode` runs on dedicated `std::thread` workers — one per
//! [`Player`]. Audio output is hard real-time and belongs on a dedicated,
//! predictable thread, not a co-op-scheduled task. The public API is
//! fully synchronous and non-blocking: methods post commands to the
//! worker and return. Embedders on any runtime (tokio, async-std, none)
//! call in via `spawn_blocking` or a channel bridge.
//!
//! ## Quick start
//!
//! ```no_run
//! use cantode::{PlayerContext, Player, AudioSource};
//! # use std::time::Duration;
//! # fn make_source() -> Box<dyn AudioSource> { unimplemented!() }
//!
//! let mut cx = PlayerContext::new()?;
//! let player = Player::new(&mut cx)?;
//!
//! let metadata = player.load(make_source())?;
//! println!("duration: {:?}", metadata.duration);
//!
//! player.play()?;
//! # std::thread::sleep(Duration::from_millis(0));
//! player.pause()?;
//! # Ok::<(), cantode::CantodeError>(())
//! ```
//!
//! [ease]: https://github.com/hpp2334/ease-music-player

#![warn(missing_docs)]
#![forbid(unsafe_code)]

pub mod context;
pub mod decoder;
pub mod error;
pub mod events;
pub mod metadata;
pub mod output;
pub mod player;
pub mod source;
pub mod state;

// ----- public re-exports -----

pub use context::{PlayerContext, PlayerContextConfig};
pub use decoder::{AudioFormat, Decoder, DecoderFactory, DecodedFrame};
pub use error::{CantodeError, Result};
pub use events::{ChannelEventSink, EventSink, NullEventSink, PlayerEvent};
pub use metadata::{probe_metadata, CoverArt, Metadata};
pub use player::{Player, PlayerConfig};
pub use source::{AudioSource, MemoryAudioSource};
pub use state::PlayerState;

pub use decoder::SymphoniaDecoderFactory;

// Note: `AudioSink`, `CpalSink`, `CpalSinkBuilder`, and `NullSink` are
// intentionally NOT re-exported. The output layer is an internal
// abstraction; embedders drive audio through `Player` and cantode selects
// the cpal backend on its own. Tests run against the real cpal host and
// require an audio device (see `tests/` for the rationale).
