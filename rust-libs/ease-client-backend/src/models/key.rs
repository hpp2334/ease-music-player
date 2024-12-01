#[derive(Debug, Clone, Copy, bitcode::Encode, bitcode::Decode, PartialEq, Eq, PartialOrd, Ord)]
pub enum DbKeyAlloc {
    Playlist,
    Music,
    Storage,
    Blob,
}
