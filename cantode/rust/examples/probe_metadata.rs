//! Probe audio files for metadata without opening an output device.
//!
//! Demonstrates the headless path: [`cantode::probe_metadata`] +
//! [`cantode::MemoryAudioSource`] on a shared [`cantode::PlayerContext`].
//! Safe to run on machines with no sound hardware — probing never touches
//! the output device, which makes it suitable for background scanning.
//!
//! ```sh
//! cargo run --example probe_metadata -- track.mp3 another.flac
//! ```

use std::{env, error::Error, fs, process::ExitCode, time::Duration};

use cantode::{AudioSource, MemoryAudioSource, PlayerContext, probe_metadata};

fn main() -> ExitCode {
    let paths: Vec<String> = env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: probe_metadata <file> [<file> ...]");
        return ExitCode::FAILURE;
    }

    // One context per application is the intended shape; it is shared by
    // every probe here (and would be shared by players, too).
    let cx = match PlayerContext::new() {
        Ok(cx) => cx,
        Err(e) => {
            eprintln!("error: creating PlayerContext failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut failed = false;
    for path in &paths {
        println!("{path}");
        if let Err(e) = probe_one(&cx, path) {
            eprintln!("  error: {e}");
            failed = true;
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// Probe one file and print a summary of its metadata.
fn probe_one(cx: &PlayerContext, path: &str) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;

    // `MemoryAudioSource` owns a copy of the bytes — the ready-made source
    // for fully-buffered media. Real embedders usually implement
    // `AudioSource` over a file or an HTTP range reader instead (see
    // examples/play_file.rs).
    let source: Box<dyn AudioSource> = Box::new(MemoryAudioSource::new(bytes));
    let meta = probe_metadata(cx, source)?;

    println!(
        "  format    : {} Hz, {} channel(s)",
        meta.format.sample_rate, meta.format.channels
    );
    println!(
        "  duration  : {}",
        meta.duration
            .map(fmt_time)
            .unwrap_or_else(|| "(unknown)".into())
    );
    println!(
        "  samples   : {}",
        meta.total_samples
            .map(|s| s.to_string())
            .unwrap_or_else(|| "(unknown)".into())
    );
    if meta.tags.is_empty() {
        println!("  tags      : (none)");
    } else {
        println!("  tags      :");
        for tag in &meta.tags {
            println!("    {} = {}", tag.key, tag.value);
        }
    }
    match &meta.cover_art {
        Some(art) => println!("  cover art : {} ({} bytes)", art.mime, art.data.len()),
        None => println!("  cover art : (none)"),
    }
    Ok(())
}

/// Format a duration as `m:ss.ss`.
fn fmt_time(d: Duration) -> String {
    let secs = d.as_secs_f64();
    format!("{}:{secs:05.2}", (secs / 60.0) as u64)
}
