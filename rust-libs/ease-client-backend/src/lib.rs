use std::sync::Arc;

use controllers::generated::dispatch_message;
use ctx::BackendContext;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub mod error;
pub(crate) mod models;
pub(crate) mod repositories;
pub(crate) mod services;
pub(crate) mod utils;

use ease_client_shared::backends::{
    app::ArgInitializeApp, connector::IConnectorNotifier, message::MessagePayload, music::MusicId,
    storage::DataSourceKey,
};
pub use ease_remote_storage::StreamFile;
use error::BResult;
use misty_async::{AsyncRuntime, IOnAsyncRuntime};
pub use services::player::{IPlayerDelegate, MusicToPlay};
use services::{app::app_bootstrap, server::load_asset};

uniffi::setup_scaffolding!();

pub struct Backend {
    cx: Arc<BackendContext>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        tracing::info!("drop Backend")
    }
}

impl Backend {
    pub fn new(rt: Arc<AsyncRuntime>, player: Arc<dyn IPlayerDelegate>) -> Self {
        let cx = Arc::new(BackendContext::new(rt, player));
        Self { cx }
    }

    pub fn init(&self, arg: ArgInitializeApp) -> BResult<()> {
        app_bootstrap(&self.cx, arg)?;
        Ok(())
    }

    pub async fn request(&self, arg: MessagePayload) -> BResult<MessagePayload> {
        let cx = self.cx.clone();
        let res = dispatch_message(&cx, arg).await;
        if let Err(ref e) = &res {
            tracing::error!("Backend request fail: {:?}", e);
        }
        res
    }

    pub async fn load_asset(
        &self,
        key: DataSourceKey,
        byte_offset: u64,
    ) -> BResult<Option<StreamFile>> {
        load_asset(&self.cx, key, byte_offset).await
    }

    pub fn serve_music_url(&self, id: MusicId) -> String {
        self.cx.asset_server().serve_music_url(id)
    }

    pub fn storage_path(&self) -> String {
        self.cx.get_storage_path()
    }
}
