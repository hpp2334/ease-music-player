use std::sync::{Arc, RwLock};

use ease_client::{to_host::connector::IConnectorHost, EaseError, EaseResult};
use ease_client_backend::{error::BResult, Backend};
use ease_client_shared::backends::{connector::IConnectorNotifier, MessagePayload};
use misty_vm::BoxFuture;
use tokio::sync::{mpsc, oneshot};

type Mp = (MessagePayload, oneshot::Sender<BResult<MessagePayload>>);
type Mtx = mpsc::Sender<Mp>;

pub struct BackendHost {
    _sender: Mtx,
    _backend: RwLock<Option<Arc<Backend>>>,
}

impl BackendHost {
    pub fn new() -> Arc<Self> {
        let (tx, mut rx) = mpsc::channel::<Mp>(10);
        let this = Arc::new(Self {
            _sender: tx,
            _backend: Default::default(),
        });
        let cloned = this.clone();

        tokio::spawn(async move {
            while let Some((payload, tx)) = rx.recv().await {
                let ret = this.backend().request(payload).await;
                let _ = tx.send(ret);
            }
        });

        cloned
    }

    pub fn has_backend(&self) -> bool {
        self._backend.read().unwrap().is_some()
    }

    pub fn set_backend(&self, backend: Arc<Backend>) {
        assert!(!self.has_backend());
        let mut w = self._backend.write().unwrap();
        *w = Some(backend);
    }

    pub fn backend(&self) -> Arc<Backend> {
        let backend = self._backend.read().unwrap();
        backend.as_ref().unwrap().clone()
    }
}

impl IConnectorHost for BackendHost {
    fn connect(&self, notifier: Arc<dyn IConnectorNotifier>) -> usize {
        self.backend().connect(notifier)
    }

    fn disconnect(&self, handle: usize) {
        self.backend().disconnect(handle);
    }

    fn request(&self, msg: MessagePayload) -> BoxFuture<EaseResult<MessagePayload>> {
        let (tx, rx) = oneshot::channel();
        let sender = self._sender.clone();
        Box::pin(async move {
            sender.send((msg, tx)).await.unwrap();
            let ret = rx
                .await
                .unwrap()
                .map_err(|e| EaseError::BackendChannelError(e.into()));
            ret
        })
    }

    fn storage_path(&self) -> String {
        self.backend().storage_path()
    }
}
