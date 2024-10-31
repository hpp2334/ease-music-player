
use ctx::BackendContext;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub mod error;
pub(crate) mod models;
pub(crate) mod repositories;
pub(crate) mod services;
pub(crate) mod utils;

pub use controllers::*;
use ease_client_shared::backends::{app::ArgInitializeApp, message::MessagePayload};
use error::BResult;
use services::app::app_bootstrap;

pub struct Backend {
    cx: BackendContext,
}

impl Backend {
    pub fn new() -> Self {
        Self {
            cx: BackendContext::new(),
        }
    }

    pub fn init(&self, arg: ArgInitializeApp) -> BResult<()> {
        app_bootstrap(&self.cx, arg)?;
        Ok(())
    }

    pub async fn request(&self, arg: MessagePayload) -> BResult<MessagePayload> {
        let res = dispatch_message(self.cx.clone(), arg).await;
        if let Err(ref e) = &res {
            tracing::error!("Backend request fail: {:?}", e);
        }
        res
    }

    pub fn port(&self) -> u16 {
        self.cx.get_server_port()
    }
}
