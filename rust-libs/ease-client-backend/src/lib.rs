use std::sync::Arc;

use ctx::BackendContext;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub mod error;
mod infra;
mod objects;
pub(crate) mod repositories;
pub(crate) mod services;
pub(crate) mod utils;

pub use objects::*;

pub use ease_remote_storage::StreamFile;
use error::BResult;

pub use crate::services::ArgInitializeApp;
use crate::{infra::init_infra, services::app_bootstrap};

uniffi::setup_scaffolding!();

#[derive(uniffi::Object)]
pub struct Backend {
    arg: ArgInitializeApp,
    cx: Arc<BackendContext>,
}

impl Drop for Backend {
    fn drop(&mut self) {
        tracing::info!("drop Backend")
    }
}

#[uniffi::export]
impl Backend {
    pub fn init(&self) -> BResult<()> {
        app_bootstrap(&self.cx, self.arg.clone())?;
        Ok(())
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
    Arc::new(Backend { cx, arg })
}
