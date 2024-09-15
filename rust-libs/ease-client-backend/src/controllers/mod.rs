use playlist::{
    cr_get_all_playlist_metas, cr_get_playlist, GetAllPlaylistMetasMsg, GetPlaylistMsg,
};

use crate::{
    core::{channel::MessageChannel, handler::HandlersBuilder},
    ctx::Context,
    generate_handler,
};

pub mod code;
pub mod music;
pub mod playlist;
pub mod storage;

pub fn build_message_channel(cx: Context) -> MessageChannel<Context> {
    let handlers = HandlersBuilder::<Context>::new()
        .add(generate_handler!(
            Context,
            GetAllPlaylistMetasMsg,
            cr_get_all_playlist_metas
        ))
        .add(generate_handler!(Context, GetPlaylistMsg, cr_get_playlist))
        .build();

    MessageChannel::<Context>::new(cx, handlers)
}
