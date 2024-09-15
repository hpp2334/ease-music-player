use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::serve::interface::IServer;

#[derive(Clone)]
pub struct Context {
    pub db_uri: String,
    pub server: Arc<dyn IServer>,
}
