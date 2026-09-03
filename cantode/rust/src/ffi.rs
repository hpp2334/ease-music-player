//! cantode's own Kotlin-facing bridge (feature `ffi`).

// Audited exception to the crate's `deny(unsafe_code)`: this module
// contains no `unsafe` blocks — the only "unsafe" is the edition-2024
// `#[unsafe(no_mangle)]` attribute on the JNI exports below, which is
// exactly as safe as the symbol names it pins.
#![allow(unsafe_code)]
//!
//! The engine owns both halves of its wire: this module is the JNI
//! surface consumed by the Kotlin facade in `cantode/kotlin`
//! (`com.kutedev.cantode.CantodeNative`). It knows nothing about any
//! embedder's business logic — the only things that cross the boundary
//! are opaque handles, transport commands, and engine observables.
//!
//! # Handles
//!
//! - **Player handle** (`jlong`): the embedder's own id for the player
//!   (the Android backend reuses its bridge handle id). The registry
//!   stores a [`Weak`] reference — the embedder's `Arc<Player>` remains
//!   the owner, so a stale Kotlin-side handle degrades to a no-op /
//!   empty poll instead of a use-after-free. No unregister call needed.
//! - **Source token** (`jlong`): sources are *built by the embedder*
//!   (they reach into its storage layer) but *played by the engine*, so
//!   they travel as pre-registered [`AudioSource`] boxes: the embedder
//!   calls [`register_source`] (plain Rust — e.g. from the backend's
//!   `player.prepareMusicSource` bridge arm), hands the token to Kotlin,
//!   which passes it to `load`/`loadAndPlay`. Taking a token consumes
//!   it.
//!
//! # Wire
//!
//! `poll` returns a small JSON snapshot (fixed-ASCII, hand-formatted —
//! no serde dependency):
//!
//! ```json
//! {"state":"LOADING","stateSeq":3,
//!  "transitions":[{"seq":1,"state":"LOADING"},{"seq":2,"state":"PLAYING"}],
//!  "positionMs":1234,"durationMs":210000}
//! ```
//!
//! `transitions` are the engine's state-history entries after `sinceSeq`
//! (see [`Player::transitions_since`]); replaying them lets a sampling
//! poller recover sub-tick excursions. `durationMs` is `null` until a
//! load completes. State names are `SCREAMING_SNAKE_CASE` — keep them in
//! sync with the Kotlin `PlayerState` enum.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, Weak};

use jni::objects::{JClass, JString};
use jni::sys::{jboolean, jlong};
use jni::JNIEnv;

use crate::state::PlayerState;
use crate::{AudioSource, Player};

// ---------------------------------------------------------------------------
// Registries (embedder-facing, plain Rust — no JNI involved)
// ---------------------------------------------------------------------------

fn players() -> &'static Mutex<HashMap<u64, Weak<Player>>> {
    static REG: OnceLock<Mutex<HashMap<u64, Weak<Player>>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

fn sources() -> &'static Mutex<HashMap<u64, Box<dyn AudioSource>>> {
    static REG: OnceLock<Mutex<HashMap<u64, Box<dyn AudioSource>>>> = OnceLock::new();
    REG.get_or_init(|| Mutex::new(HashMap::new()))
}

fn next_token() -> u64 {
    static COUNTER: OnceLock<Mutex<u64>> = OnceLock::new();
    let mut c = COUNTER.get_or_init(|| Mutex::new(0)).lock().unwrap();
    *c += 1;
    *c
}

/// Register a player under `key` (the embedder's own id for it — the
/// Android backend reuses its bridge handle id so one number addresses
/// the player on both bridges). Holds a [`Weak`] — the embedder's
/// `Arc<Player>` stays the owner; dropping it deadens the handle.
pub fn register_player(key: u64, player: &Arc<Player>) {
    players().lock().unwrap().insert(key, Arc::downgrade(player));
}

/// The live player for `key`, if its owner still holds it.
pub fn player(key: u64) -> Option<Arc<Player>> {
    players().lock().unwrap().get(&key).and_then(Weak::upgrade)
}

/// Register a loadable source; returns the opaque token handed to
/// `CantodeNative.load`/`loadAndPlay` (consuming on use).
pub fn register_source(source: Box<dyn AudioSource>) -> u64 {
    let token = next_token();
    sources().lock().unwrap().insert(token, source);
    token
}

/// Consume the source registered under `token`.
pub fn take_source(token: u64) -> Option<Box<dyn AudioSource>> {
    sources().lock().unwrap().remove(&token)
}

// ---------------------------------------------------------------------------
// JNI exports (Kotlin: com.kutedev.cantode.CantodeNative)
// ---------------------------------------------------------------------------

fn state_str(s: PlayerState) -> &'static str {
    match s {
        PlayerState::Idle => "IDLE",
        PlayerState::Loading => "LOADING",
        PlayerState::Paused => "PAUSED",
        PlayerState::Playing => "PLAYING",
        PlayerState::Buffering => "BUFFERING",
        PlayerState::Ended => "ENDED",
        PlayerState::Error => "ERROR",
    }
}

/// The poll snapshot as JSON. Empty string signals "no player / gone" —
/// the Kotlin poller treats it as a skipped tick.
fn poll_json(p: &Player, since_seq: u64) -> String {
    let (state_seq, transitions) = p.transitions_since(since_seq);
    let transitions: Vec<String> = transitions
        .iter()
        .map(|(seq, st)| format!(r#"{{"seq":{seq},"state":"{}"}}"#, state_str(*st)))
        .collect();
    format!(
        r#"{{"state":"{}","stateSeq":{state_seq},"transitions":[{}],"positionMs":{},"durationMs":{}}}"#,
        state_str(p.state()),
        transitions.join(","),
        p.position().as_millis(),
        p.duration().map(|d| d.as_millis().to_string()).unwrap_or_else(|| "null".into()),
    )
}

/// Batched engine snapshot: state + transition history since `sinceSeq`
/// + position + duration. Call at ~10 Hz.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_cantode_CantodeNative_poll<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    handle: jlong,
    since_seq: jlong,
) -> JString<'local> {
    let json = player(handle as u64)
        .map(|p| poll_json(&p, since_seq.max(0) as u64))
        .unwrap_or_default();
    env.new_string(json).unwrap_or_default()
}

/// Begin or resume playback.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_cantode_CantodeNative_play(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if let Some(p) = player(handle as u64) {
        let _ = p.play();
    }
}

/// Pause playback.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_cantode_CantodeNative_pause(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if let Some(p) = player(handle as u64) {
        let _ = p.pause();
    }
}

/// Stop and drop the loaded source (back to `Idle`).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_cantode_CantodeNative_stop(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if let Some(p) = player(handle as u64) {
        let _ = p.stop();
    }
}

/// Seek to `ms` from source start. No-op without a loaded source.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_cantode_CantodeNative_seek(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    ms: jlong,
) {
    if let Some(p) = player(handle as u64) {
        let _ = p.seek(std::time::Duration::from_millis(ms.max(0) as u64));
    }
}

/// Set linear gain (`1.0` = unity, `0.0` = silent).
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_cantode_CantodeNative_setVolume(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    volume: jni::sys::jfloat,
) {
    if let Some(p) = player(handle as u64) {
        let _ = p.set_volume(volume);
    }
}

/// Load the pre-registered source (`sourceToken` from
/// [`register_source`]) and start playing the moment it completes.
/// Blocks the calling thread until the source is open (network-bound) —
/// call from a worker dispatcher. Returns `true` on success; a failure
/// transitions the engine to `Error`, which the next poll reports.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_kutedev_cantode_CantodeNative_loadAndPlay(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
    source_token: jlong,
) -> jboolean {
    let Some(p) = player(handle as u64) else {
        return 0;
    };
    let Some(source) = take_source(source_token as u64) else {
        return 0;
    };
    match p.load_and_play(source) {
        Ok(_) => 1,
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_strings_are_screaming_snake() {
        assert_eq!(state_str(PlayerState::Idle), "IDLE");
        assert_eq!(state_str(PlayerState::Loading), "LOADING");
        assert_eq!(state_str(PlayerState::Paused), "PAUSED");
        assert_eq!(state_str(PlayerState::Playing), "PLAYING");
        assert_eq!(state_str(PlayerState::Buffering), "BUFFERING");
        assert_eq!(state_str(PlayerState::Ended), "ENDED");
        assert_eq!(state_str(PlayerState::Error), "ERROR");
    }
}
