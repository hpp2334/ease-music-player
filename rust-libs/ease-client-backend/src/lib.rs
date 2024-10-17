use ctx::BackendContext;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub mod error;
pub(crate) mod models;
pub(crate) mod repositories;
pub(crate) mod services;
pub(crate) mod utils;

pub use controllers::*;
use ease_client_shared::backends::{app::ArgInitializeApp, message::{IMessage, MessagePayload}};
use error::BResult;
use services::app::app_bootstrap;

pub struct Backend {
    cx: BackendContext,
}

impl Backend {
    pub fn new(arg: ArgInitializeApp) -> BResult<Self> {
        let cx = app_bootstrap(arg)?;
        Ok(Backend { cx })
    }

    pub async fn send(&self, arg: MessagePayload) -> BResult<MessagePayload>
    {
        dispatch_message(self.cx.clone(), arg).await
    }
}
