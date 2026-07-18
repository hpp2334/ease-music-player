use std::sync::Arc;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub mod error;
mod infra;
mod objects;
pub(crate) mod repositories;
mod services;
mod streaming_server;
pub(crate) mod utils;

pub use objects::*;

pub use ease_remote_storage::StreamFile;
use error::BResult;

pub use crate::services::ArgInitializeApp;
use crate::{
    ctx::BackendContext,
    infra::init_infra,
    services::{app_bootstrap, app_destroy},
};

uniffi::setup_scaffolding!();

#[derive(uniffi::Object)]
pub struct Backend {
    arg: ArgInitializeApp,
    cx: Arc<BackendContext>,
    streaming_server: std::sync::Mutex<Option<streaming_server::StreamingServer>>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        tracing::info!("drop Backend")
    }
}

#[uniffi::export]
impl Backend {
    pub fn init(&self) -> BResult<()> {
        let cx = self.cx.clone();
        let arg = self.arg.clone();
        ease_client_tokio::tokio_runtime().block_on(async move {
            app_bootstrap(&cx, arg).await
        })?;

        let server = streaming_server::StreamingServer::start(self.cx.weak());
        tracing::info!("streaming server started at {}", server.base_url());
        *self.streaming_server.lock().unwrap() = Some(server);
        Ok(())
    }

    pub fn deinit(&self) -> BResult<()> {
        *self.streaming_server.lock().unwrap() = None;
        ease_client_tokio::tokio_runtime().block_on(async {
            app_destroy(&self.cx).await
        })
    }

    /// Returns the streaming HTTP server's base URL (e.g.
    /// `http://127.0.0.1:54321`), or `None` if `init()` has not been
    /// called yet. JavaFX MediaPlayer points at `<base_url>/music/:id`.
    pub fn streaming_base_url(&self) -> Option<String> {
        self.streaming_server
            .lock()
            .unwrap()
            .as_ref()
            .map(|s| s.base_url().to_string())
    }
}

impl Backend {
    pub fn get_context(&self) -> &BackendContext {
        &self.cx
    }

    pub fn storage_path(&self) -> String {
        self.cx.get_storage_path()
    }
}

#[uniffi::export]
pub fn create_backend(arg: ArgInitializeApp) -> Arc<Backend> {
    let cx = Arc::new(BackendContext::new());
    init_infra(&arg.app_document_dir);
    Arc::new(Backend {
        cx,
        arg,
        streaming_server: std::sync::Mutex::new(None),
    })
}

#[uniffi::export]
pub fn ease_log(msg: &str) {
    tracing::info!("{}", msg);
}

#[uniffi::export]
pub fn ease_error(msg: &str) {
    tracing::error!("{}", msg);
}
