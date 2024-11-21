use std::fmt::Debug;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use super::generated::Code;

pub trait IMessage {
    const CODE: Code;
    type Argument: Debug + Serialize + DeserializeOwned + Send + Sync + 'static;
    type Return: Debug + Serialize + DeserializeOwned + Send + Sync + 'static;
}

#[derive(Deserialize, Serialize)]
pub struct MessagePayload {
    pub code: Code,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

macro_rules! define_message {
    ($msg: ident, $code: expr, $arg: ty, $ret: ty) => {
        pub struct $msg {}
        impl crate::backends::message::IMessage for $msg {
            const CODE: Code = $code;
            type Argument = $arg;
            type Return = $ret;
        }
    };
}

pub fn decode_message_payload<T>(arg: Vec<u8>) -> T
where
    T: Serialize + DeserializeOwned,
{
    let ret = rmp_serde::from_slice(arg.as_slice()).unwrap();
    ret
}

pub fn encode_message_payload<T>(arg: T) -> Vec<u8>
where
    T: Serialize + DeserializeOwned,
{
    let ret = rmp_serde::to_vec(&arg).unwrap();
    ret
}
