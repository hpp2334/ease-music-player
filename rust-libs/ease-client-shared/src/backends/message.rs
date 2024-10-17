use serde::{de::DeserializeOwned, Deserialize, Serialize};


pub trait IMessage {
    const CODE: u32;
    type Argument: Serialize + DeserializeOwned + Send + 'static;
    type Return: Serialize + DeserializeOwned + Send + 'static;
}

#[derive(Deserialize, Serialize)]
pub struct MessagePayload {
    pub code: u32,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
}

#[macro_export]
macro_rules! define_message {
    ($msg: ident, $code: expr, $arg: ty, $ret: ty) => {
        pub struct $msg {}
        impl crate::backends::message::IMessage for $msg {
            const CODE: u32 = $code as u32;
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
