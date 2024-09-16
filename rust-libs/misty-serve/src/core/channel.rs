use super::{
    handler::Handlers,
    result::ChannelResult,
    schema::{decode_message_payload, encode_message_payload, IMessage},
};

pub struct MessageChannel<S>
where
    S: Clone + Send + Sync + 'static,
{
    cx: S,
    handlers: Handlers<S>,
}

impl<S> MessageChannel<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new(cx: S, handlers: Handlers<S>) -> Self {
        Self { cx, handlers }
    }

    pub async fn send<M>(&self, arg: M::Argument) -> ChannelResult<M::Return>
    where
        M: IMessage,
    {
        let handler = self.handlers.get(M::CODE)?;
        let arg = encode_message_payload(arg)?;
        let ret = handler.process(self.cx.clone(), arg).await?;
        let ret = decode_message_payload::<M::Return>(ret)?;
        Ok(ret)
    }

    pub fn context(&self) -> &S {
        &self.cx
    }
}
