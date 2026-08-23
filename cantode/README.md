# cantode

A cross-platform **audio engine** for Rust: decode, output, and metadata
probing behind a pluggable, trait-based API. Designed as a future
replacement for media3 ExoPlayer in [Ease Music Player][ease], but with no
dependency on that project — `cantode` is a standalone, runtime-agnostic
library.

[ease]: https://github.com/hpp2334/ease-music-player

## Design

- **Trait-based where it matters.** `AudioSource` and `Decoder` are
  public traits that embedders can substitute; `EventSink` too. The
  output layer (`AudioSink`, `CpalSink`) is **internal** — cantode
  always drives the system audio device via cpal, and callers don't pick
  or implement sinks. This keeps the audio path honest: every test, every
  embedder, every CI run goes through the real cpal backend.
- **No async runtime.** The engine runs on dedicated `std::thread`
  workers (one per player). Audio output is hard real-time and belongs
  on a dedicated, predictable thread, not a co-op-scheduled task. The
  public API is fully synchronous and non-blocking — methods post
  commands to the worker and return immediately. Embedders on any
  runtime (tokio, async-std, none) call in via `spawn_blocking` or a
  channel bridge.
- **Shared resources via `PlayerContext`.** One context owns the cpal
  `Host`, the shared `DecoderFactory`, an optional global `EventSink`,
  and a registry of live players. Players are created against a context
  (`Player::new(&mut cx)`) and may outlive it.
- **Pure-function state machine.** `state::transition(state, event)` has
  no I/O and is unit-tested in isolation from the worker.
- **Metadata without playback.** `probe_metadata(&cx, source)` decodes
  just enough of a source to read tags, duration, and cover art — never
  touching the output device.

## Supported platforms

| Platform | Output backend |
|---|---|
| Android (`arm64-v8a`) | cpal → oboe → AAudio |
| Linux | cpal → ALSA / PulseAudio / JACK |
| macOS | cpal → CoreAudio |
| Windows | cpal → WASAPI / ASIO |

Decoding is pure-Rust via [`symphonia`][symphonia] (MP3, FLAC, Vorbis,
Opus, WAV, AAC, ISOMP4), so cross-compiling to `aarch64-linux-android`
needs no C toolchain beyond the NDK itself.

[symphonia]: https://github.com/pdeljanov/Symphonia

## Quick example

```rust
use std::time::Duration;
use cantode::{PlayerContext, Player, AudioSource};

// Implement AudioSource for your byte source (file, HTTP range, ...).
// cantode ships MemoryAudioSource for tests and simple embedders.
fn make_source() -> Box<dyn AudioSource> {
    // ...
#   unimplemented!()
}

# fn main() -> cantode::Result<()> {
let cx = PlayerContext::new()?;
let player = Player::new(&cx)?;

let metadata = player.load(make_source())?;
println!("duration: {:?}", metadata.duration);

player.play()?;
std::thread::sleep(Duration::from_secs(2));
player.pause()?;
# Ok(())
# }
```

## Examples

Runnable, end-to-end examples live in [`examples/`](./examples):

| Example | What it shows | Output device |
|---|---|---|
| `probe_metadata` | Headless probing: tags, duration, cover art via `probe_metadata` + `MemoryAudioSource` | not needed |
| `play_file` | Event-driven playback: a custom file-backed `AudioSource`, `ChannelEventSink` events, progress line | required |
| `transport_controls` | Poll-driven scripting: 10 Hz `state()`/`position()` polling, seek / pause / resume / stop | required |

```sh
cargo run --example probe_metadata -- path/to/track.mp3
cargo run --example play_file -- path/to/track.mp3          # plays out loud
cargo run --example play_file -- path/to/track.mp3 0.1      # quiet
cargo run --example transport_controls -- path/to/track.mp3
```

The two playback examples drive the **real** default output device —
same philosophy as the test suite (see [Testing requirements](#testing-requirements)).

## Feature flags

| Feature | Default | Effect |
|---|---|---|
| `sink-cpal` | yes | Drive the system audio device via cpal (always uses the default output device) |

That's the only feature flag. The symphonia decoder is unconditional —
it's the built-in decoder and always available. To substitute a
different decoder, implement `Decoder` + `DecoderFactory` and pass your
factory to `PlayerContextConfig::decoder_factory`; no feature flag needed.

Disabling `sink-cpal` yields a headless build that can still decode and
probe metadata, but `Player::load` will return
`CantodeError::Sink("no sink backend enabled...")`. There is no
`NullSink` — tests run against the real cpal device.

## Testing requirements

Integration tests under `tests/` open the host's real audio output
device (volume is set to 0.0 so the suite is silent). They panic via
`common::require_audio_device()` when the host has no output device.
On a headless CI runner, install a virtual loopback/null sink:

- **Linux (PulseAudio):** `pactl load-module module-null-sink`
- **Linux (PipeWire):** `pactl load-module module-null-sink` (pipewire-pulse)
  or run with the pipewire-alsa / pipewire-jack bridge
- **macOS:** install [BlackHole](https://github.com/ExistentialAudio/BlackHole)
- **Windows:** WASAPI always exposes a software endpoint; no setup needed

## License

Dual-licensed under **MIT** or **Apache-2.0**, at your option. See
[`LICENSE-MIT`](./LICENSE-MIT) and [`LICENSE-APACHE`](./LICENSE-APACHE).
