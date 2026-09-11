use std::sync::Arc;

pub mod bridge;
pub mod controllers;
pub mod ctx;
pub mod error;
mod infra;
mod objects;
pub mod repositories;
pub mod services;
pub mod utils;

pub use objects::*;

pub use ease_remote_storage::StreamFile;
use error::BResult;

pub use crate::services::ArgInitializeApp;
use crate::{
    ctx::BackendContext,
    infra::init_infra,
    services::{app_bootstrap, app_destroy},
};

pub struct Backend {
    pub(crate) arg: ArgInitializeApp,
    cx: Arc<BackendContext>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        tracing::info!("drop Backend")
    }
}

impl Backend {
    pub async fn init_async(&self) -> BResult<()> {
        let cx = self.cx.clone();
        let arg = self.arg.clone();
        app_bootstrap(&cx, arg).await?;
        Ok(())
    }

    /// Legacy sync entrypoint — must NOT be called from inside a tokio
    /// runtime context. Used by tests; the bridge dispatcher uses
    /// [`Backend::init_async`] instead.
    pub fn init(&self) -> BResult<()> {
        ease_client_tokio::tokio_runtime().block_on(self.init_async())
    }

    pub async fn deinit_async(&self) -> BResult<()> {
        app_destroy(&self.cx).await
    }

    pub fn deinit(&self) -> BResult<()> {
        ease_client_tokio::tokio_runtime().block_on(self.deinit_async())
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

pub fn create_backend(arg: ArgInitializeApp) -> Arc<Backend> {
    let cx = Arc::new(BackendContext::new());
    init_infra(&arg.app_document_dir);
    Arc::new(Backend { cx, arg })
}

pub fn ease_log(msg: &str) {
    tracing::info!("{}", msg);
}

pub fn ease_error(msg: &str) {
    tracing::error!("{}", msg);
}

// The Android embedder half (JNI surface, tur engine binding, the
// `PluginEngineHost` implementation) lives in the `ease-client-android`
// crate — this crate stays platform-agnostic and host-testable. Its
// engine seam is [`services::plugin_manager::PluginEngineHost`], attached
// by that crate's `bindPluginRuntime` trampoline.
