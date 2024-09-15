use core::{channel::MessageChannel, result::ChannelResult, schema::IMessage};

use ctx::Context;

pub(crate) mod controllers;
pub(crate) mod core;
pub(crate) mod ctx;
pub(crate) mod models;
pub(crate) mod repositories;
pub(crate) mod services;
pub(crate) mod utils;

pub use controllers::*;

pub struct Backend {
    cx: Context,
    channel: MessageChannel<Context>,
}

impl Backend {
    pub fn new(document_dir: String) -> Self {
        let db_uri = document_dir + "/app.db";

        let cx = Context { db_uri };
        let channel = build_message_channel(cx.clone());
        Backend { cx, channel }
    }

    pub async fn send<M>(&self, arg: M::Argument) -> ChannelResult<M::Return>
    where
        M: IMessage,
    {
        self.channel.send::<M>(self.cx.clone(), arg).await
    }
}
