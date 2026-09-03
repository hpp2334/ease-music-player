//! Scripted transport controls driven by polling instead of events.
//!
//! Demonstrates the "other" consumption pattern: no `EventSink` at all,
//! just a ~10 Hz poll of the lock-free `state()` / `position()`
//! accessors (this is what the Android embedder of cantode does).
//!
//! Requires an output device; audio plays out loud. Use a track of at
//! least ~10 s so every phase of the script has time to run:
//! play 3 s → seek to the middle → 2 s → pause 1 s → resume → play to
//! the end.
//!
//! ```sh
//! cargo run --example transport_controls -- track.mp3
//! ```

use std::{
    env,
    error::Error,
    fs,
    io::{self, Write},
    process::ExitCode,
    thread,
    time::{Duration, Instant},
};

use cantode::{AudioSource, CantodeError, MemoryAudioSource, Player, PlayerContext, PlayerState};

/// Poll cadence for the progress line.
const POLL: Duration = Duration::from_millis(100);

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
    let Some(path) = env::args().nth(1) else {
        return Err("usage: transport_controls <file>".into());
    };

    let cx = PlayerContext::new()?;
    let player = Player::new(&cx)?;

    // Whole file in memory (contrast with the streaming `FileAudioSource`
    // in examples/play_file.rs).
    let bytes = fs::read(&path)?;
    let source: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(bytes));

    let meta = match player.load(source) {
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
    let Some(duration) = meta.duration else {
        return Err("this example needs a source with a known duration".into());
    };
    println!(
        "loaded: {} Hz / {} ch, duration {}",
        meta.format.sample_rate,
        meta.format.channels,
        fmt_time(duration)
    );

    player.set_volume(0.5)?;
    player.play()?;
    wait_for_state(&player, PlayerState::Playing, Duration::from_secs(2));
    println!(
        "script: play 3 s → seek to {} → 2 s → pause 1 s → resume → play to end",
        fmt_time(duration / 2)
    );

    let started = Instant::now();
    let deadline = started + duration + Duration::from_secs(10);
    let mut seeked = false;
    let mut paused = false;

    loop {
        thread::sleep(POLL);
        print!(
            "\r  {} / {} [{:?}]  ",
            fmt_time(player.position()),
            fmt_time(duration),
            player.state()
        );
        io::stdout().flush()?;

        let t = started.elapsed();
        if !seeked && t >= Duration::from_secs(3) {
            seeked = true;
            // Seek targets land on codec frame boundaries; the return
            // value is where the decoder actually landed (exact for WAV,
            // snapped for MP3 & co). Note that `seek` waits for the
            // worker's reply — while playing, the worker spends most of
            // its time feeding the device ring buffer, so the call can
            // lag wall-clock by roughly the sink's buffered duration.
            let actual = player.seek(duration / 2)?;
            println!(
                "\nseeked: requested {}, landed on {}",
                fmt_time(duration / 2),
                fmt_time(actual)
            );
        } else if seeked && !paused && t >= Duration::from_secs(5) {
            paused = true;
            player.pause()?;
            wait_for_state(&player, PlayerState::Paused, Duration::from_secs(2));
            println!("\npaused (state {:?}) — sleeping 1 s…", player.state());
            thread::sleep(Duration::from_secs(1));
            player.play()?;
            wait_for_state(&player, PlayerState::Playing, Duration::from_secs(2));
            println!("resumed (state {:?})", player.state());
        }

        if player.state() == PlayerState::Ended {
            println!("\nended at {}", fmt_time(player.position()));
            break;
        }
        if Instant::now() >= deadline {
            println!("\nsafety deadline reached before the stream ended");
            break;
        }
    }

    // `stop` posts a command and returns immediately, so poll until the
    // state actually settles back to `Idle`.
    player.stop()?;
    wait_for_state(&player, PlayerState::Idle, Duration::from_secs(2));
    println!("after stop: state {:?}", player.state());
    Ok(())
}

/// Poll `player.state()` until it reaches `want` or `timeout` elapses.
///
/// All `Player` accessors are lock-free, so polling this fast is cheap.
fn wait_for_state(player: &Player, want: PlayerState, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        if player.state() == want {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

/// Format a duration as `m:ss.ss`.
fn fmt_time(d: Duration) -> String {
    let secs = d.as_secs_f64();
    format!("{}:{secs:05.2}", (secs / 60.0) as u64)
}
