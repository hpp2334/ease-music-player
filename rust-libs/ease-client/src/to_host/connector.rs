use std::sync::Arc;

use ease_client_shared::backends::{connector::IConnectorNotifier, message::MessagePayload};
use misty_vm::{misty_to_host, BoxFuture};

use crate::error::EaseResult;

pub trait IConnectorHost: Send + Sync + 'static {
    fn connect(&self, notifier: Arc<dyn IConnectorNotifier>) -> usize;
    fn disconnect(&self, handle: usize);
    fn request(&self, msg: MessagePayload) -> BoxFuture<EaseResult<MessagePayload>>;
    fn storage_path(&self) -> String;
}

misty_to_host!(ConnectorHostService, IConnectorHost);
