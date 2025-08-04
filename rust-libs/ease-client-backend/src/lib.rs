use std::sync::Arc;

use ctx::BackendContext;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub mod error;
pub(crate) mod models;
mod objects;
pub(crate) mod repositories;
pub(crate) mod services;
pub(crate) mod utils;

pub use objects::*;

pub use ease_remote_storage::StreamFile;
use error::BResult;

use crate::services::app_bootstrap;
pub use crate::services::ArgInitializeApp;

uniffi::setup_scaffolding!();

#[derive(uniffi::Object)]
pub struct Backend {
    cx: Arc<BackendContext>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        tracing::info!("drop Backend")
    }
}

impl Backend {
    pub fn new() -> Self {
        let cx = Arc::new(BackendContext::new());
        Self { cx }
    }

    pub fn get_context(&self) -> &BackendContext {
        &self.cx
    }

    pub fn init(&self, arg: ArgInitializeApp) -> BResult<()> {
        app_bootstrap(&self.cx, arg)?;
        Ok(())
    }

    pub fn storage_path(&self) -> String {
        self.cx.get_storage_path()
    }
}
