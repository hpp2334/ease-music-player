pub mod loc;
mod serve;

use std::sync::Arc;

use ease_client_shared::backends::storage::DataSourceKey;
use ease_remote_storage::StreamFile;
use serve::get_stream_file_cover_by_music_id;
pub use serve::start_server;

use crate::{ctx::BackendContext, error::BResult};

pub(crate) async fn load_asset(
    cx: &Arc<BackendContext>,
    key: DataSourceKey,
) -> BResult<Option<StreamFile>> {
    match key {
        DataSourceKey::Music { id } => {
            unimplemented!()
        }
        DataSourceKey::Cover { id } => get_stream_file_cover_by_music_id(cx, id).await,
        DataSourceKey::AnyEntry { entry } => {
            unimplemented!()
        }
    }
}
