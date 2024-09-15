use serde::{de::DeserializeOwned, Serialize};

pub trait IMessage {
    const CODE: u32;
    type Argument: DeserializeOwned + Send + 'static;
    type Return: Serialize + Send + 'static;
}

#[macro_export]
macro_rules! define_message {
    ($msg: ident, $code: expr, $arg: ty, $ret: ty) => {
        pub struct $msg {}
        impl crate::core::schema::IMessage for $msg {
            const CODE: u32 = $code as u32;
            type Argument = $arg;
            type Return = $ret;
        }
    };
}
