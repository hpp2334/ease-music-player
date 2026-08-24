//! Play one audio file through the real output device, event-driven.
//!
//! Demonstrates:
//!
//! - a custom [`cantode::AudioSource`] (`FileAudioSource`) — how an
//!   embedder plugs its own byte source into cantode,
//! - per-player events via [`cantode::ChannelEventSink`] (subscribe
//!   **before** creating the player so nothing is missed),
//! - the transport API: `load` → `set_volume` → `play`, plus lock-free
//!   `state()` / `position()` polling alongside the event stream.
//!
//! Requires an output device; audio plays out loud.
//!
//! ```sh
//! cargo run --example play_file -- track.mp3        # default volume 0.8
//! cargo run --example play_file -- track.mp3 0.1    # quiet
//! ```

use std::{
    env,
    error::Error,
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    path::Path,
    process::ExitCode,
    sync::{Arc, mpsc::RecvTimeoutError},
    time::Duration,
};

use cantode::{
    AudioSource, CantodeError, ChannelEventSink, Player, PlayerConfig, PlayerContext, PlayerEvent,
    PlayerState,
};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);
    let Some(path) = args.next() else {
        return Err("usage: play_file <file> [volume 0.0..1.0]".into());
    };
    let volume: f32 = match args.next().as_deref() {
        Some(v) => v
            .parse()
            .map_err(|e| format!("invalid volume `{v}`: {e}"))?,
        None => 0.8,
    };

    // Subscribe to the event sink first, then hand it to the player —
    // events emitted between subscribe and player creation simply queue
    // up, so nothing is lost.
    let sink = Arc::new(ChannelEventSink::new(256));
    let events = sink.subscribe();

    let cx = PlayerContext::new()?;
    let player = Player::with_config(
        &cx,
        PlayerConfig {
            event_sink: Some(sink),
        },
    )?;

    // `load` blocks until the decoder is open and metadata is ready; it
    // does NOT start playback (the player sits in `Paused` afterwards).
    let meta = match player.load(Box::new(FileAudioSource::open(Path::new(&path))?)) {
        Ok(meta) => meta,
        Err(CantodeError::NoOutputDevice) => {
            return Err(
                "no output device — connect one (or install a virtual loopback \
                        device; see README \"Testing requirements\")"
                    .into(),
            );
        }
        Err(CantodeError::UnsupportedFormat(detail)) => {
            return Err(format!("unsupported format: {detail}").into());
        }
        Err(e) => return Err(e.into()),
    };
    let duration = meta.duration;
    println!(
        "loaded: {} Hz / {} ch, duration {}",
        meta.format.sample_rate,
        meta.format.channels,
        duration.map(fmt_time).unwrap_or_else(|| "(unknown)".into())
    );

    player.set_volume(volume)?;
    player.play()?;

    // Event loop. `PositionChanged` arrives ~10 Hz while playing; the
    // 500 ms timeout doubles as a fallback poll (`state()` / `position()`
    // are lock-free) for stretches where no events flow.
    let mut partial_line = false;
    loop {
        match events.recv_timeout(Duration::from_millis(500)) {
            Ok(PlayerEvent::StateChanged(state)) => {
                if partial_line {
                    println!();
                    partial_line = false;
                }
                println!("state: {state:?}");
                if state == PlayerState::Ended {
                    break;
                }
            }
            Ok(PlayerEvent::PositionChanged(pos)) => {
                let total = duration.map(fmt_time).unwrap_or_else(|| "--:--".into());
                print!("\r  {} / {total} ", fmt_time(pos));
                io::stdout().flush()?;
                partial_line = true;
            }
            Ok(PlayerEvent::MetadataReady(_)) => {
                // Emitted once per successful load, right before `Paused`.
                // We already printed the metadata from `load`'s return
                // value, so nothing to do here.
            }
            Ok(PlayerEvent::Error(e)) => {
                if partial_line {
                    println!();
                    partial_line = false;
                }
                eprintln!("player error: {e}");
                if player.state() == PlayerState::Error {
                    break;
                }
            }
            Ok(PlayerEvent::Ended) => {
                if partial_line {
                    println!();
                }
                println!("ended");
                break;
            }
            Err(RecvTimeoutError::Timeout) => {
                print!(
                    "\r  {} [{:?}] ",
                    fmt_time(player.position()),
                    player.state()
                );
                io::stdout().flush()?;
                partial_line = true;
            }
            Err(RecvTimeoutError::Disconnected) => {
                println!("\nevent channel closed (player dropped)");
                break;
            }
        }
    }

    // Back to `Idle`: posts a command, returns immediately. Dropping
    // `player` afterwards posts shutdown and joins the worker thread.
    player.stop()?;
    Ok(())
}

/// A file-backed [`AudioSource`] — the minimal custom source.
///
/// `std::fs::File` already implements `Read + Seek` and is
/// `Send + Sync`, so all cantode asks for is a known length.
struct FileAudioSource {
    file: File,
    len: Option<u64>,
}

impl FileAudioSource {
    fn open(path: &Path) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata().ok().map(|m| m.len());
        Ok(Self { file, len })
    }
}

impl Read for FileAudioSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.file.read(buf)
    }
}

impl Seek for FileAudioSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.file.seek(pos)
    }
}

impl AudioSource for FileAudioSource {
    fn len(&self) -> Option<u64> {
        self.len
    }
}

/// Format a duration as `m:ss.ss`.
fn fmt_time(d: Duration) -> String {
    let secs = d.as_secs_f64();
    format!("{}:{secs:05.2}", (secs / 60.0) as u64)
}
