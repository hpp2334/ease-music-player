#![allow(unused_imports)]
use crate::backends::connector::*;
use crate::backends::music::*;
use crate::backends::player::*;
use crate::backends::playlist::*;
use crate::backends::storage::*;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum Code {
    OnConnect,
    GetMusic,
    UpdateMusicLyric,
    EnableTimeToPause,
    DisableTimeToPause,
    PlayerCurrent,
    PlayerPlaymode,
    PlayerDurations,
    PlayMusic,
    PausePlayer,
    PlayNext,
    PlayPrevious,
    StopPlayer,
    PlayerSeek,
    UpdatePlaymode,
    ResumePlayer,
    OnPlayerEvent,
    GetPlaylist,
    UpdatePlaylist,
    CreatePlaylist,
    AddMusicsToPlaylist,
    RemoveMusicFromPlaylist,
    RemovePlaylist,
    UpsertStorage,
    GetRefreshToken,
    RemoveStorage,
    TestStorage,
    ListStorageEntryChildren,
}
define_message! {
    OnConnectMsg,
    Code::OnConnect,
    (),
    ()
}
define_message! {
    GetMusicMsg,
    Code::GetMusic,
    MusicId,
    Option<Music>
}
define_message! {
    UpdateMusicLyricMsg,
    Code::UpdateMusicLyric,
    ArgUpdateMusicLyric,
    ()
}
define_message! {
    EnableTimeToPauseMsg,
    Code::EnableTimeToPause,
    std::time::Duration,
    ()
}
define_message! {
    DisableTimeToPauseMsg,
    Code::DisableTimeToPause,
    (),
    ()
}
define_message! {
    PlayerCurrentMsg,
    Code::PlayerCurrent,
    (),
    Option<PlayerCurrentPlaying>
}
define_message! {
    PlayerPlaymodeMsg,
    Code::PlayerPlaymode,
    (),
    PlayMode
}
define_message! {
    PlayerDurationsMsg,
    Code::PlayerDurations,
    (),
    PlayerDurations
}
define_message! {
    PlayMusicMsg,
    Code::PlayMusic,
    ArgPlayMusic,
    ()
}
define_message! {
    PausePlayerMsg,
    Code::PausePlayer,
    (),
    ()
}
define_message! {
    PlayNextMsg,
    Code::PlayNext,
    (),
    ()
}
define_message! {
    PlayPreviousMsg,
    Code::PlayPrevious,
    (),
    ()
}
define_message! {
    StopPlayerMsg,
    Code::StopPlayer,
    (),
    ()
}
define_message! {
    PlayerSeekMsg,
    Code::PlayerSeek,
    u64,
    ()
}
define_message! {
    UpdatePlaymodeMsg,
    Code::UpdatePlaymode,
    PlayMode,
    ()
}
define_message! {
    ResumePlayerMsg,
    Code::ResumePlayer,
    (),
    ()
}
define_message! {
    OnPlayerEventMsg,
    Code::OnPlayerEvent,
    PlayerDelegateEvent,
    ()
}
define_message! {
    GetPlaylistMsg,
    Code::GetPlaylist,
    PlaylistId,
    Option<Playlist>
}
define_message! {
    UpdatePlaylistMsg,
    Code::UpdatePlaylist,
    ArgUpdatePlaylist,
    ()
}
define_message! {
    CreatePlaylistMsg,
    Code::CreatePlaylist,
    ArgCreatePlaylist,
    PlaylistId
}
define_message! {
    AddMusicsToPlaylistMsg,
    Code::AddMusicsToPlaylist,
    ArgAddMusicsToPlaylist,
    ()
}
define_message! {
    RemoveMusicFromPlaylistMsg,
    Code::RemoveMusicFromPlaylist,
    ArgRemoveMusicFromPlaylist,
    ()
}
define_message! {
    RemovePlaylistMsg,
    Code::RemovePlaylist,
    PlaylistId,
    ()
}
define_message! {
    UpsertStorageMsg,
    Code::UpsertStorage,
    ArgUpsertStorage,
    ()
}
define_message! {
    GetRefreshTokenMsg,
    Code::GetRefreshToken,
    String,
    String
}
define_message! {
    RemoveStorageMsg,
    Code::RemoveStorage,
    StorageId,
    ()
}
define_message! {
    TestStorageMsg,
    Code::TestStorage,
    ArgUpsertStorage,
    StorageConnectionTestResult
}
define_message! {
    ListStorageEntryChildrenMsg,
    Code::ListStorageEntryChildren,
    StorageEntryLoc,
    ListStorageEntryChildrenResp
}
