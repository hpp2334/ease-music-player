use std::sync::Arc;

use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Context {
    pub db_uri: String,
}
