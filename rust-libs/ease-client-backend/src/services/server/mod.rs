mod asset;
pub(self) mod chunks;
mod serve;

use std::sync::Arc;

pub use asset::AssetServer;
use ease_client_shared::backends::storage::DataSourceKey;
use ease_remote_storage::StreamFile;
use serve::{
    get_stream_file_by_loc, get_stream_file_by_music_id, get_stream_file_cover_by_music_id,
};

use crate::{ctx::BackendContext, error::BResult};

pub(crate) async fn load_asset(
    cx: &BackendContext,
    key: DataSourceKey,
    byte_offset: u64,
) -> BResult<Option<StreamFile>> {
    let cx = cx.clone();
    match key {
        DataSourceKey::Music { id } => get_stream_file_by_music_id(&cx, id, byte_offset).await,
        DataSourceKey::Cover { id } => {
            get_stream_file_cover_by_music_id(&cx, id, byte_offset).await
        }
        DataSourceKey::AnyEntry { entry } => get_stream_file_by_loc(&cx, entry, byte_offset).await,
    }
}
