use crate::{
    ctx::BackendContext,
    error::BResult,
    services::{app::load_preference_data, player::on_connect_for_player},
};

pub(crate) async fn ci_on_connect(cx: &BackendContext, _arg: ()) -> BResult<()> {
    let data = load_preference_data(cx);

    on_connect_for_player(cx, data.playmode)?;
    Ok(())
}
