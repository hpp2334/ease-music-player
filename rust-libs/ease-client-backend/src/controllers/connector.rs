use std::sync::Arc;

use futures::try_join;

use crate::{
    ctx::BackendContext,
    error::BResult,
    services::{
        music::notify_time_to_pause, player::on_connect_for_player,
        playlist::notify_all_playlist_abstracts, storage::notify_storages,
    },
};

pub(crate) async fn ci_on_connect(cx: &Arc<BackendContext>, _arg: ()) -> BResult<()> {
    let data = cx.database_server().load_preference()?;

    try_join! {
        on_connect_for_player(cx, data.playmode),
        notify_all_playlist_abstracts(cx),
        notify_storages(cx),
        notify_time_to_pause(cx),
    }?;
    Ok(())
}
