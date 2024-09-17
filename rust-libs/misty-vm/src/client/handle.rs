use std::sync::{Arc, Weak};

use crate::resources::MistyResourceManager;

use super::{MistyClientId, MistyClientInner};

#[derive(Clone, Copy)]
pub struct MistyReadonlyClientHandle<'a> {
    pub(crate) inner: &'a Arc<MistyClientInner>,
}

#[derive(Clone, Copy)]
pub struct MistyClientHandle<'a> {
    pub(crate) inner: &'a Arc<MistyClientInner>,
}

#[derive(Clone)]
pub struct MistyClientAccessor {
    pub(crate) inner: Weak<MistyClientInner>,
}

pub struct MistyReadonlyClientHandlePod {
    inner: Arc<MistyClientInner>,
}

impl MistyClientAccessor {
    pub fn get(&self) -> Option<MistyReadonlyClientHandlePod> {
        let handle: Option<Arc<MistyClientInner>> = self.inner.upgrade();

        if let Some(handle) = handle {
            Some(MistyReadonlyClientHandlePod { inner: handle })
        } else {
            None
        }
    }
}

impl MistyReadonlyClientHandlePod {
    pub fn handle(&self) -> MistyReadonlyClientHandle<'_> {
        MistyReadonlyClientHandle { inner: &self.inner }
    }
}

impl<'a> MistyClientHandle<'a> {
    pub fn id(&self) -> MistyClientId {
        self.inner.id
    }

    pub fn resource_manager(&self) -> &MistyResourceManager {
        &self.inner.resource_manager
    }

    pub fn is_destroyed(&self) -> bool {
        self.inner.is_destroyed()
    }
}

impl<'a> MistyReadonlyClientHandle<'a> {
    pub fn id(&self) -> MistyClientId {
        self.inner.id
    }

    pub fn schedule<E>(
        &self,
        handler: impl FnOnce(MistyClientHandle) -> Result<(), E> + Send + Sync + 'static,
    ) where
        E: std::fmt::Display,
    {
        self.inner
            .schedule_manager
            .enqueue(&self.inner.signal_emitter, handler);
    }

    pub fn accessor(&self) -> MistyClientAccessor {
        MistyClientAccessor {
            inner: Arc::downgrade(self.inner),
        }
    }

    pub fn resource_manager(&self) -> &MistyResourceManager {
        &self.inner.resource_manager
    }

    pub fn is_destroyed(&self) -> bool {
        self.inner
            .destroyed
            .load(std::sync::atomic::Ordering::SeqCst)
    }
}
