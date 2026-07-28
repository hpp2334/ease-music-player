//! Opaque-handle table for long-lived objects the bridge creates.
//!
//! The Kotlin side never sees the underlying Rust types — it receives a
//! `u64` handle ID at construction (e.g. `backend.create` returns `{ handle: 1 }`)
//! and passes it back on every subsequent call. This indirection lets us
//! keep the JSON request shape uniform (`{ method, args, handle? }`) and
//! preserves the existing `Arc<Backend>` / `Arc<PlayerHandle>` semantics
//! that the underlying `ct_*` / `cts_*` functions expect.

use std::sync::{atomic::{AtomicU64, Ordering}, Mutex, OnceLock};

use crate::{Backend, PlayerContextHandle, PlayerHandle};

/// Monotonic ID generator; first handle is 1 (0 reserved as "no handle").
static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);

/// The handle table itself. Lazily initialized on first access.
static HANDLE_TABLE: OnceLock<Mutex<std::collections::HashMap<u64, HandleEntry>>> = OnceLock::new();

fn table() -> &'static Mutex<std::collections::HashMap<u64, HandleEntry>> {
    HANDLE_TABLE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
}

/// Discriminated entry stored in the table. Matched by [`get`] helpers.
pub(crate) enum HandleEntry {
    Backend(std::sync::Arc<Backend>),
    PlayerContext(std::sync::Arc<PlayerContextHandle>),
    Player(std::sync::Arc<PlayerHandle>),
}

/// Register a new entry and return its handle ID.
pub(crate) fn register(entry: HandleEntry) -> u64 {
    let id = NEXT_HANDLE_ID.fetch_add(1, Ordering::Relaxed);
    table().lock().unwrap().insert(id, entry);
    id
}

pub(crate) fn get_backend(id: u64) -> Option<std::sync::Arc<Backend>> {
    let guard = table().lock().unwrap();
    match guard.get(&id)? {
        HandleEntry::Backend(b) => Some(b.clone()),
        _ => None,
    }
}

pub(crate) fn get_player_context(id: u64) -> Option<std::sync::Arc<PlayerContextHandle>> {
    let guard = table().lock().unwrap();
    match guard.get(&id)? {
        HandleEntry::PlayerContext(c) => Some(c.clone()),
        _ => None,
    }
}

pub(crate) fn get_player(id: u64) -> Option<std::sync::Arc<PlayerHandle>> {
    let guard = table().lock().unwrap();
    match guard.get(&id)? {
        HandleEntry::Player(p) => Some(p.clone()),
        _ => None,
    }
}

/// Remove a handle from the table. Returns true if it was present.
#[allow(dead_code)]
pub(crate) fn remove(id: u64) -> bool {
    table().lock().unwrap().remove(&id).is_some()
}
