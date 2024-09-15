use std::{any::Any, collections::HashMap, sync::Arc};

use crate::ctx::Context;

use super::{
    handler::{Handlers, IHandler},
    result::ChannelResult,
    schema::IMessage,
};

pub struct MessageChannel<S>
where
    S: Clone + Send + Sync + 'static,
{
    cx: Context,
    handlers: Handlers<S>,
}

impl<S> MessageChannel<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new(cx: Context, handlers: Handlers<S>) -> Self {
        Self { cx, handlers }
    }

    pub async fn receive<M>(&self, state: S, arg: M::Argument) -> ChannelResult<M::Return>
    where
        M: IMessage,
    {
        let handler = self.handlers.get(M::CODE)?;
        let arg: Box<dyn Any + Send> = Box::new(arg);
        let ret = handler.process(state, arg).await?;
        let ret = *ret.downcast::<M::Return>().unwrap();
        Ok(ret)
    }
}
