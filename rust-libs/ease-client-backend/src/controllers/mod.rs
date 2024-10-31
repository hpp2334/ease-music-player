use ease_client_shared::backends::code::Code;
use ease_client_shared::backends::message::decode_message_payload;
use ease_client_shared::backends::message::encode_message_payload;
use ease_client_shared::backends::message::IMessage;
use ease_client_shared::backends::message::MessagePayload;
use music::*;
use playlist::*;
use preference::*;
use storage::*;

use ease_client_shared::backends::music::*;
use ease_client_shared::backends::playlist::*;
use ease_client_shared::backends::preference::*;
use ease_client_shared::backends::storage::*;
use tracing::instrument;

use crate::ctx::BackendContext;
use crate::error::BError;
use crate::error::BResult;

pub mod music;
pub mod playlist;
mod preference;
pub mod storage;

#[instrument]
fn trace_request<M: IMessage>(code: Code, arg: &<M as IMessage>::Argument) {
    tracing::trace!("request {:?}: {:?}", code, arg)
}

#[instrument]
fn trace_response<M: IMessage>(code: Code, arg: &<M as IMessage>::Return) {
    tracing::trace!("response {:?}: {:?}", code, arg)
}

macro_rules! generate_dispatch_message {
    ($($m: ident, $h: ident),*) => {
            pub(crate) async fn dispatch_message(cx: BackendContext, arg: MessagePayload) -> BResult<MessagePayload> {
            $(
                if <$m as IMessage>::CODE == arg.code {
                    let code = arg.code;
                    let arg = decode_message_payload::<<$m as IMessage>::Argument>(arg.payload);
                    trace_request::<$m>(code, &arg);
                    let ret = $h(cx, arg).await?;
                    trace_response::<$m>(code, &ret);
                    let ret = encode_message_payload(ret);
                    let ret = MessagePayload {
                        code,
                        payload: ret,
                    };
                    return Ok(ret);
                }
            )*
            return Err(BError::NoSuchMessage(arg.code));
            }
    };
}

generate_dispatch_message!(
    // Playlist
    GetAllPlaylistAbstractsMsg,
    cr_get_all_playlist_abstracts,
    GetPlaylistMsg,
    cr_get_playlist,
    CreatePlaylistMsg,
    cc_create_playlist,
    UpdatePlaylistMsg,
    cu_update_playlist,
    AddMusicsToPlaylistMsg,
    cu_add_musics_to_playlist,
    RemoveMusicsFromPlaylistMsg,
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
    RemoveStorageMsg,
    cd_remove_storage,
    TestStorageMsg,
    cr_test_storage,
    ListStorageEntryChildrenMsg,
    cr_list_storage_entry_children,
    // Preference
    GetPreferenceMsg,
    cr_get_preference,
    UpdatePreferencePlaymodeMsg,
    cu_update_preference_playmode
);
