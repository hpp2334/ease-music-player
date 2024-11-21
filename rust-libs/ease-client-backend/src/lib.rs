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
    app::ArgInitializeApp, connector::IConnectorNotifier, message::MessagePayload,
    storage::DataSourceKey,
};
use ease_remote_storage::StreamFile;
use error::BResult;
use misty_async::{AsyncRuntime, IOnAsyncRuntime};
pub use services::player::{IPlayerDelegate, MusicToPlay};
use services::{app::app_bootstrap, server::load_asset};

uniffi::setup_scaffolding!();

pub struct Backend {
    cx: Arc<BackendContext>,
}

impl Backend {
    pub fn new(rt: Arc<AsyncRuntime>, player: Arc<dyn IPlayerDelegate>) -> Self {
        Self {
            cx: Arc::new(BackendContext::new(rt, player)),
        }
    }

    pub fn init(&self, arg: ArgInitializeApp) -> BResult<()> {
        app_bootstrap(&self.cx, arg)?;
        Ok(())
    }

    pub fn flush_spawned_locals(&self) {
        self.cx.async_runtime().flush_local_spawns();
    }

    pub fn connect(&self, notifier: Arc<dyn IConnectorNotifier>) -> usize {
        self.cx.connect(notifier)
    }

    pub fn disconnect(&self, handle: usize) {
        self.cx.disconnect(handle);
    }

    pub async fn request(&self, arg: MessagePayload) -> BResult<MessagePayload> {
        let cx = self.cx.clone();
        let res = dispatch_message(&cx, arg).await;
        if let Err(ref e) = &res {
            tracing::error!("Backend request fail: {:?}", e);
        }
        res
    }

    pub fn request_from_host(&self, arg: MessagePayload) {
        let cx = self.cx.clone();
        self.cx
            .async_runtime()
            .spawn(async move {
                let res = dispatch_message(&cx, arg).await;
                if let Err(ref e) = &res {
                    tracing::error!("Backend request fail: {:?}", e);
                }
            })
            .detach();
    }

    pub async fn load_asset(&self, key: DataSourceKey) -> BResult<Option<StreamFile>> {
        let cx = self.cx.clone();
        load_asset(&self.cx, key).await
    }

    pub fn port(&self) -> u16 {
        self.cx.get_server_port()
    }

    pub fn storage_path(&self) -> String {
        self.cx.get_storage_path()
    }
}

impl IOnAsyncRuntime for Backend {
    fn flush_spawned_locals(&self) {
        self.flush_spawned_locals();
    }
}
