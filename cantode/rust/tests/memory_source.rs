//! Tests for [`MemoryAudioSource`]: Read/Seek correctness and length report.

#![allow(clippy::unused_io_amount)]

use std::io::{Read, Seek, SeekFrom};

use cantode::{AudioSource, MemoryAudioSource};

#[test]
fn len_reports_buffer_size() {
    let src = MemoryAudioSource::new(vec![1, 2, 3, 4, 5]);
    assert_eq!(src.len(), Some(5));
    assert!(!src.is_empty());
    assert!(!src.is_infinite());
}

#[test]
fn empty_source_is_empty() {
    let src = MemoryAudioSource::new(Vec::new());
    assert_eq!(src.len(), Some(0));
    assert!(src.is_empty());
}

#[test]
fn read_returns_prefix_bytes() {
    let mut src = MemoryAudioSource::new(vec![10, 20, 30, 40]);
    let mut buf = [0u8; 2];
    // First read: two bytes from the front.
    let n = src.read(&mut buf).unwrap();
    assert_eq!(n, 2);
    assert_eq!(buf, [10, 20]);

    // Second read: the remaining two bytes.
    let n = src.read(&mut buf).unwrap();
    assert_eq!(n, 2);
    assert_eq!(buf, [30, 40]);

    // Third read: EOF.
    let n = src.read(&mut buf).unwrap();
    assert_eq!(n, 0);
}

#[test]
fn seek_from_start_updates_position() {
    let mut src = MemoryAudioSource::new(vec![1, 2, 3, 4, 5]);
    let pos = src.seek(SeekFrom::Start(3)).unwrap();
    assert_eq!(pos, 3);

    let mut buf = [0u8; 2];
    src.read_exact(&mut buf).unwrap();
    assert_eq!(buf, [4, 5]);
}

#[test]
fn seek_from_end_relative() {
    let mut src = MemoryAudioSource::new(vec![1, 2, 3, 4, 5]);
    let pos = src.seek(SeekFrom::End(-1)).unwrap();
    assert_eq!(pos, 4);

    let mut buf = [0u8; 1];
    src.read_exact(&mut buf).unwrap();
    assert_eq!(buf, [5]);
}

#[test]
fn seek_current_advances() {
    let mut src = MemoryAudioSource::new(vec![1, 2, 3, 4, 5, 6]);
    let mut buf = [0u8; 2];
    src.read_exact(&mut buf).unwrap(); // pos now 2

    let pos = src.seek(SeekFrom::Current(2)).unwrap();
    assert_eq!(pos, 4);

    src.read_exact(&mut buf).unwrap();
    assert_eq!(buf, [5, 6]);
}

#[test]
fn clone_is_independent() {
    let mut a = MemoryAudioSource::new(vec![1, 2, 3]);
    let mut b = a.clone();
    let mut buf = [0u8; 1];
    a.read_exact(&mut buf).unwrap();
    assert_eq!(buf, [1]);
    // b is unaffected by a's read.
    b.read_exact(&mut buf).unwrap();
    assert_eq!(buf, [1]);
}
