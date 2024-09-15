use core::{channel::MessageChannel, result::ChannelResult, schema::IMessage};

use controllers::build_message_channel;
use ctx::Context;

pub(crate) mod controllers;
pub(crate) mod core;
pub(crate) mod ctx;
pub(crate) mod models;
pub(crate) mod repositories;
pub(crate) mod serve;
pub(crate) mod utils;

pub struct Backend {
    cx: Context,
    channel: MessageChannel<Context>,
}

impl Backend {
    pub fn new() -> Self {
        let cx = Context { serverport: 0 };
        let channel = build_message_channel(cx.clone());
        Backend { cx, channel }
    }

    pub async fn send<M>(&self, arg: M::Payload) -> ChannelResult<M::Return>
    where
        M: IMessage,
    {
        self.channel.receive::<M>(self.cx.clone(), arg).await
    }
}
