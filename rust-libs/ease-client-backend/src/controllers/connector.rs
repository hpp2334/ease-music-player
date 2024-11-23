use std::sync::Arc;

use futures::try_join;

use crate::{
    ctx::BackendContext,
    error::BResult,
    services::{
        app::load_preference_data, player::on_connect_for_player,
        playlist::notify_all_playlist_abstracts, storage::notify_storages,
    },
};

pub(crate) async fn ci_on_connect(cx: &Arc<BackendContext>, _arg: ()) -> BResult<()> {
    let data = load_preference_data(cx);

    try_join! {
        on_connect_for_player(cx, data.playmode),
        notify_all_playlist_abstracts(cx),
        notify_storages(cx),
    }?;
    Ok(())
}
