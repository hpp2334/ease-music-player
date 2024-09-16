use misty_serve::{channel::MessageChannel, generate_handlers};
pub use music::*;
pub use playlist::*;
pub use storage::*;

use ease_client_shared::backends::music::*;
use ease_client_shared::backends::playlist::*;
use ease_client_shared::backends::storage::*;

use crate::ctx::Context;

pub mod music;
pub mod playlist;
pub mod storage;

pub fn build_message_channel(cx: Context) -> MessageChannel<Context> {
    let handlers = generate_handlers!(
        Context,
        // Playlist
        GetAllPlaylistMetasMsg,
        cr_get_all_playlist_metas,
        UpdatePlaylistMsg,
        ccu_upsert_playlist,
        AddMusicsToPlaylistMsg,
        cu_add_musics_to_playlist,
        RemoveMusicsToPlaylistMsg,
        cd_remove_music_from_playlist,
        RemovePlaylistMsg,
        cd_remove_playlist,
        // Music
        GetMusicMsg,
        cr_get_music,
        UpdateMusicDurationMsg,
        cu_update_music_duration,
        UpdateMusicCoverMsg,
        cu_update_music_cover,
        UpdateMusicLyricMsg,
        cu_update_music_lyric,
        // Storage
        UpsertStorageMsg,
        ccu_upsert_storage,
        ListStorageMsg,
        cr_list_storage,
        GetStorageMsg,
        cr_get_storage,
        GetToRemoveStorageRefsMsg,
        cr_get_to_remove_storage_refs,
        RemoveStorageMsg,
        cd_remove_storage,
        TestStorageMsg,
        cr_test_storage,
        ListStorageEntryChildrenMsg,
        cr_list_storage_entry_children
    );

    MessageChannel::<Context>::new(cx, handlers)
}
