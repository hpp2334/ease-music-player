use std::{future::Future, sync::Arc};

use ease_client_backend::{error::BResult, Backend};
use ease_client_shared::backends::{app::ArgInitializeApp, message::MessagePayload};
use misty_vm::{misty_to_host, BoxFuture};
use tokio::sync::{mpsc, oneshot};

use crate::error::{EaseError, EaseResult};

pub trait IConnectorHost: Send + Sync + 'static {
    fn init(&self, arg: ArgInitializeApp) -> EaseResult<()>;
    fn request(&self, msg: MessagePayload) -> BoxFuture<EaseResult<MessagePayload>>;
    fn port(&self) -> u16;
}

misty_to_host!(ConnectorHostService, IConnectorHost);

type Mp = (MessagePayload, oneshot::Sender<BResult<MessagePayload>>);
type Mrx = mpsc::Receiver<Mp>;
type Mtx = mpsc::Sender<Mp>;

pub struct ConnectorHostImpl {
    backend: Arc<Backend>,
    sender: Mtx,
}

impl ConnectorHostImpl {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel::<Mp>(10);
        let backend: Arc<Backend> = Arc::new(Backend::new());
        Self::start(rx, backend.clone());

        Self {
            backend,
            sender: tx,
        }
    }

    fn start(mut rx: Mrx, backend: Arc<Backend>) {
        tokio::spawn(async move {
            while let Some((payload, tx)) = rx.recv().await {
                let ret = backend.request(payload).await;
                let _ = tx.send(ret);
            }
        });
    }
}

impl IConnectorHost for ConnectorHostImpl {
    fn request(&self, msg: MessagePayload) -> BoxFuture<EaseResult<MessagePayload>> {
        let (tx, rx) = oneshot::channel();
        let sender = self.sender.clone();
        Box::pin(async move {
            let _ = sender.send((msg, tx));
            let ret = rx
                .await
                .unwrap()
                .map_err(|e| EaseError::BackendChannelError(e));
            ret
        })
    }

    fn init(&self, arg: ArgInitializeApp) -> EaseResult<()> {
        self.backend.init(arg)?;
        Ok(())
    }

    fn port(&self) -> u16 {
        self.backend.port()
    }
}
