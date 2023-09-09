use num_traits::FromPrimitive;
use serde::{Deserialize, Serialize};

use crate::modules::{StorageId, StorageType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct StorageModel {
    pub id: StorageId,
    pub addr: String,
    pub alias: Option<String>,
    pub username: String,
    pub password: String,
    pub is_anonymous: bool,
    pub typ: i32,
}

pub struct Storage {
    pub(super) model: StorageModel,
}

impl Storage {
    pub fn id(&self) -> StorageId {
        self.model.id
    }

    pub fn addr(&self) -> &str {
        self.model.addr.as_str()
    }

    pub fn alias(&self) -> &Option<String> {
        &self.model.alias
    }

    pub fn username(&self) -> &str {
        &self.model.username
    }

    pub fn password(&self) -> &str {
        &self.model.password
    }

    pub fn is_anonymous(&self) -> bool {
        self.model.is_anonymous
    }

    pub fn typ(&self) -> StorageType {
        StorageType::from_i32(self.model.typ).unwrap()
    }

    pub fn display_name(&self) -> String {
        let alias = self.alias().clone().unwrap_or_default();
        if !alias.is_empty() {
            alias
        } else {
            self.addr().to_string()
        }
    }
}
