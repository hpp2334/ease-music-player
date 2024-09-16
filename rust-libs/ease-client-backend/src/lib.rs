use ctx::Context;

pub(crate) mod controllers;
pub(crate) mod ctx;
pub(crate) mod models;
pub(crate) mod repositories;
pub(crate) mod services;
pub(crate) mod utils;

pub use controllers::*;
use ease_client_shared::backends::app::ArgInitializeApp;
use misty_serve::{channel::MessageChannel, result::ChannelResult, schema::IMessage};
use services::app::app_bootstrap;

pub struct Backend {
    channel: MessageChannel<Context>,
}

impl Backend {
    pub fn new(arg: ArgInitializeApp) -> anyhow::Result<Self> {
        let cx = app_bootstrap(arg)?;

        let channel = build_message_channel(cx.clone());
        Ok(Backend { channel })
    }

    pub async fn send<M>(&self, arg: M::Argument) -> ChannelResult<M::Return>
    where
        M: IMessage,
    {
        self.channel.send::<M>(arg).await
    }
}
