use num_derive::{FromPrimitive, ToPrimitive};
use serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Default,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    FromPrimitive,
    ToPrimitive,
    uniffi::Enum,
)]
pub enum PlayMode {
    #[default]
    Single,
    SingleLoop,
    List,
    ListLoop,
}
