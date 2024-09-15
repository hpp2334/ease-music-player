use futures::future::BoxFuture;
use std::{any::Any, collections::HashMap, future::Future, sync::Arc};

use super::{
    result::{ChannelError, ChannelResult},
    schema::IMessage,
};

pub type HandlerPayload = Vec<u8>;

pub trait IHandler<S>: Send + Sync + 'static {
    fn process(&self, state: S, arg: HandlerPayload) -> BoxFuture<anyhow::Result<HandlerPayload>>;
}

#[derive(Clone)]
pub struct BoxedHandler<S>
where
    S: Clone + Send + Sync + 'static,
{
    f: Arc<dyn IHandler<S>>,
}

impl<S> BoxedHandler<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new<H>(handler: H) -> Self
    where
        H: IHandler<S>,
    {
        Self {
            f: Arc::new(handler),
        }
    }

    pub async fn process(&self, state: S, arg: HandlerPayload) -> ChannelResult<HandlerPayload> {
        let ret = self.f.process(state, arg).await;
        match ret {
            Ok(ret) => Ok(ret),
            Err(e) => Err(ChannelError::OtherError(e)),
        }
    }
}

pub struct Handlers<S>
where
    S: Clone + Send + Sync + 'static,
{
    registry: HashMap<u32, BoxedHandler<S>>,
}

pub struct HandlersBuilder<S>
where
    S: Clone + Send + Sync + 'static,
{
    registry: HashMap<u32, BoxedHandler<S>>,
}

impl<S> HandlersBuilder<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn new() -> Self {
        Self {
            registry: Default::default(),
        }
    }

    pub fn add<H>(mut self, (code, handler): (u32, H)) -> Self
    where
        H: IHandler<S>,
    {
        if self.registry.contains_key(&code) {
            let name = std::any::type_name::<H>();
            panic!("code {code} has registered, handler is {name}");
        }
        self.registry.insert(code, BoxedHandler::new(handler));
        self
    }

    pub fn build(self) -> Handlers<S> {
        Handlers {
            registry: self.registry,
        }
    }
}

impl<S> Handlers<S>
where
    S: Clone + Send + Sync + 'static,
{
    pub fn get(&self, code: u32) -> ChannelResult<BoxedHandler<S>> {
        let h = self.registry.get(&code);
        match h {
            Some(h) => Ok(h.clone()),
            None => Err(ChannelError::HandlerNotFound(code)),
        }
    }
}

#[macro_export]
macro_rules! generate_handler {
    ($stype: ident, $m: ident, $h: ident) => {
        {
            mod __misty_handler__ {
                pub struct $m;
                impl crate::core::handler::IHandler<super::$stype> for $m {
                    fn process(
                        &self,
                        state: super::$stype,
                        arg: crate::core::handler::HandlerPayload,
                    ) -> futures::future::BoxFuture<anyhow::Result<crate::core::handler::HandlerPayload>>
                    {
                        Box::pin(async {
                            let arg = crate::core::schema::decode_message_payload::<<super::$m as crate::core::schema::IMessage>::Argument>(arg)?;
                            let ret = super::$h(state, arg).await;
                            let ret = match ret {
                                Ok(ret) => crate::core::schema::encode_message_payload(ret)?,
                                Err(e) => return Err(e),
                            };
                            Ok(ret)
                        })
                    }
                }
            }
            (<$m as crate::core::schema::IMessage>::CODE as u32, __misty_handler__::$m)
        }
    }
}

#[macro_export]
macro_rules! generate_handlers {
    ($stype: ident, $($m: ident, $h: ident),*) => {
        {
        crate::core::handler::HandlersBuilder::<$stype>::new()
           $(

            .add(crate::generate_handler!(
                $stype,
                $m,
                $h
            ))
           )*
           .build()
        }
    };
}
