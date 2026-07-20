//! In-memory [`AudioSource`] backed by a [`std::io::Cursor`].
//!
//! Useful for unit tests (no filesystem dependencies) and for embedders
//! that already hold the full media buffer in RAM (e.g. cached cover art
//! blobs that happen to be full tracks, small sound effects, ...).

use std::io::{self, Cursor, Read, Seek, SeekFrom};

use crate::AudioSource;

/// An [`AudioSource`] backed by a `Vec<u8>` via [`std::io::Cursor`].
///
/// Cheap to construct, fully seekable, and reports a known length — the
/// ideal source for tests and small fully-buffered media.
#[derive(Debug, Clone)]
pub struct MemoryAudioSource {
    cursor: Cursor<Vec<u8>>,
}

impl MemoryAudioSource {
    /// Create a new source that owns a copy of the given bytes.
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    /// Create a new source from an owned byte slice of any kind.
    pub fn from_slice(slice: &[u8]) -> Self {
        Self::new(slice.to_vec())
    }
}

impl Read for MemoryAudioSource {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.cursor.read(buf)
    }
}

impl Seek for MemoryAudioSource {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(pos)
    }
}

impl AudioSource for MemoryAudioSource {
    fn len(&self) -> Option<u64> {
        Some(u64::try_from(self.cursor.get_ref().len()).unwrap_or(u64::MAX))
    }
}
