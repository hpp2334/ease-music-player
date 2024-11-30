use std::sync::{Arc, RwLock};

use ease_client::{to_host::connector::IConnectorHost, EaseError, EaseResult};
use ease_client_backend::Backend;
use ease_client_shared::backends::{connector::IConnectorNotifier, music::MusicId, MessagePayload};
use misty_vm::BoxFuture;

pub struct BackendHost {
    _backend: RwLock<Option<Arc<Backend>>>,
}

impl BackendHost {
    pub fn new() -> Arc<Self> {
        let this = Arc::new(Self {
            _backend: Default::default(),
        });
        let cloned = this.clone();

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

    pub fn reset_backend(&self) {
        assert!(self.has_backend());
        let mut w = self._backend.write().unwrap();
        *w = None;
    }

    pub fn backend(&self) -> Arc<Backend> {
        let backend = self._backend.read().unwrap();
        backend.as_ref().unwrap().clone()
    }
    pub fn try_backend(&self) -> Option<Arc<Backend>> {
        let backend = self._backend.read().unwrap();
        backend.clone()
    }
}

impl IConnectorHost for BackendHost {
    fn connect(&self, notifier: Arc<dyn IConnectorNotifier>) -> usize {
        self.backend().connect(notifier)
    }

    fn disconnect(&self, handle: usize) {
        self.backend().disconnect(handle);
    }

    fn serve_music_url(&self, id: MusicId) -> String {
        self.backend().serve_music_url(id)
    }

    fn request(&self, msg: MessagePayload) -> BoxFuture<EaseResult<MessagePayload>> {
        Box::pin(async move {
            self.backend()
                .request(msg)
                .await
                .map_err(|e| EaseError::BackendChannelError(e.into()))
        })
    }

    fn storage_path(&self) -> String {
        self.backend().storage_path()
    }
}
