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
//!   │   └─ SymphoniaDecoder      └─ CpalSink              │
//!   ├─────────────────────────────────────────────────────┤
//!   │  AudioSource (trait: Read + Seek + Send + Sync)     │
//!   │   ├─ MemoryAudioSource (tests)                      │
//!   │   └─ BufferedSource (network streaming sessions)    │
//!   └─────────────────────────────────────────────────────┘
//! ```
//!
//! Every layer is a trait; default implementations ship in the box.
//! Embedders substitute their own `AudioSource` (HTTP range
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
//! call in via `spawn_blocking` or a channel bridge. For byte sources,
//! [`BufferedSource`] performs that bridging for you: implement the
//! non-blocking [`RemoteAudioSource`] session trait (`open` / `request` /
//! `close` — one long-lived request per session, demand-told) on your
//! own runtime, and cantode owns the buffering/retrying itself.
//!
//! ## Quick start
//!
//! ```no_run
//! use cantode::{PlayerContext, Player, AudioSource};
//! # use std::time::Duration;
//! # fn make_source() -> Box<dyn AudioSource> { unimplemented!() }
//!
//! let cx = PlayerContext::new()?;
//! let player = Player::new(&cx)?;
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
//! Runnable, end-to-end versions of this — plus metadata probing,
//! event-driven playback, and transport-control scripting — live in the
//! `examples/` directory.
//!
//! [ease]: https://github.com/hpp2334/ease-music-player

#![warn(missing_docs)]
// `deny` (not `forbid`) so the `ffi` module — the JNI boundary, whose
// export attributes are `unsafe(...)` attributes in edition 2024 — can
// carry an explicit, audited `allow`. Every other module still denies.
#![deny(unsafe_code)]

pub mod context;
pub mod decoder;
#[cfg(feature = "ffi")]
pub mod ffi;
pub mod error;
pub mod events;
pub mod metadata;
pub mod output;
pub mod player;
pub mod source;
pub mod state;

// ----- public re-exports -----

pub use context::{PlayerContext, PlayerContextConfig};
pub use decoder::{AudioFormat, DecodedFrame, Decoder, DecoderFactory};
pub use error::{CantodeError, Result};
pub use events::{ChannelEventSink, EventSink, NullEventSink, PlayerEvent};
pub use metadata::{CoverArt, Metadata, Tag, probe_metadata};
pub use output::{AudioSink, AudioSinkFactory};
pub use player::{Player, PlayerConfig};
pub use source::{
    AudioSource, BufferedSource, MemoryAudioSource, Pushed, Readiness, RemoteAudioSource,
    StreamReply,
};
pub use state::PlayerState;

pub use decoder::SymphoniaDecoderFactory;

// Note: `CpalSink`, `CpalSinkBuilder`, and `NullSink` are intentionally
// NOT re-exported. The output *destination* stays an embedder-provided
// [`AudioSink`] via [`PlayerConfig::audio_sink_factory`]; the default
// cpal-backed sink cantode builds for itself is internal. Tests run
// against the real cpal host by default and require an audio device (see
// `tests/` for the rationale).
