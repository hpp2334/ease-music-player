//! tur plugin runtime — in-memory bridge between the ease backend and the
//! tur engine.
//!
//! This module wires the engine (linked as an rlib into
//! `libease_client_backend.so`) to the ease backend's services via the
//! process-wide [`crate::BACKEND_CONTEXT`] OnceLock. The bridge exposes
//! one synthetic JS module to plugins:
//!
//! - `ease:storage` — synchronous KV operations backed by the same
//!   SQLite store that holds the music / playlist data. Each call is a
//!   short `block_on` on the shared tokio runtime; the engine thread
//!   stalls for the indexed `IN (...)` query (~ms-scale, dominated by
//!   SQLite). Real-time rendering is unaffected for plugin workloads.
//!
//! Plugins carry their own domain data in event payloads (e.g. the
//! `music:play` event includes `title`), so no separate music-metadata
//! bridge is needed for the initial playcount plugin.
//!
//! Other tur modules (`tur:std`, `tur:animation`, `tur:net`) are
//! registered by the standard plugin set installed alongside
//! [`EaseMusicPlugin`] in `plugin_jni::create_ease_plugin_engine`.

pub mod plugin;
pub mod storage_bridge;
pub mod plugin_jni;

pub use plugin::EaseMusicPlugin;
