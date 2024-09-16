use serde::{de::DeserializeOwned, Serialize};

use super::result::ChannelResult;

pub trait IMessage {
    const CODE: u32;
    type Argument: Serialize + DeserializeOwned + Send + 'static;
    type Return: Serialize + DeserializeOwned + Send + 'static;
}

#[macro_export]
macro_rules! define_message {
    ($msg: ident, $code: expr, $arg: ty, $ret: ty) => {
        pub struct $msg {}
        impl misty_serve::schema::IMessage for $msg {
            const CODE: u32 = $code as u32;
            type Argument = $arg;
            type Return = $ret;
        }
    };
}

pub fn decode_message_payload<T>(arg: Vec<u8>) -> ChannelResult<T>
where
    T: Serialize + DeserializeOwned,
{
    let ret = rmp_serde::from_slice(arg.as_slice())?;
    Ok(ret)
}

pub fn encode_message_payload<T>(arg: T) -> ChannelResult<Vec<u8>>
where
    T: Serialize + DeserializeOwned,
{
    let ret = rmp_serde::to_vec(&arg)?;
    Ok(ret)
}
