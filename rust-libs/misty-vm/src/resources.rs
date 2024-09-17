use std::{
    collections::HashMap,
    fmt::Debug,
    ops::Deref,
    sync::{atomic::AtomicU64, Arc, RwLock, Weak},
};

use serde::{Deserialize, Serialize};

pub enum ResourceUpdateAction {
    Insert(MistyResourceId, Vec<u8>),
    Remove(MistyResourceId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MistyResourceId(u64);

impl MistyResourceId {
    pub fn wrap(id: u64) -> Self {
        Self(id)
    }
    pub fn invalid() -> Self {
        Self(0)
    }
}

impl Default for MistyResourceId {
    fn default() -> Self {
        Self::invalid()
    }
}

const _: () = {
    static ALLOCATED: AtomicU64 = AtomicU64::new(1);
    impl MistyResourceId {
        pub fn alloc() -> Self {
            let id = ALLOCATED.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            return Self(id);
        }
    }
    impl Deref for MistyResourceId {
        type Target = u64;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }
};

#[derive(Debug)]
enum ToFlushResourceAction {
    Insert(MistyResourceHandle),
    Remove,
}

impl PartialEq for ToFlushResourceAction {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Remove, Self::Remove) => true,
            (Self::Insert(l0), Self::Insert(r0)) => l0.id() == r0.id(),
            _ => false,
        }
    }
}

struct MistyResourceManagerStore {
    pending_actions: Arc<RwLock<Option<HashMap<MistyResourceId, ToFlushResourceAction>>>>,
    weak_map: RwLock<HashMap<MistyResourceId, Weak<MistyResourceHandleInner>>>,
}

pub struct MistyResourceManager {
    store: Arc<MistyResourceManagerStore>,
}

impl Debug for MistyResourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceManager").finish()
    }
}

#[derive(Debug, Clone)]
struct MistyResourceHandleInner {
    id: MistyResourceId,
    store_ref: Weak<MistyResourceManagerStore>,
    buf: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct MistyResourceHandle {
    ptr: Arc<MistyResourceHandleInner>,
}

impl MistyResourceHandle {
    pub fn id(&self) -> MistyResourceId {
        self.ptr.id
    }

    pub fn load(&self) -> &Vec<u8> {
        &self.ptr.buf
    }
}

impl Drop for MistyResourceHandleInner {
    fn drop(&mut self) {
        let store_ref = self.store_ref.upgrade();
        if store_ref.is_none() {
            return;
        }
        let store_ref = store_ref.unwrap();

        {
            let mut writter = store_ref.pending_actions.write().unwrap();
            let writer = writter.as_mut().unwrap();
            if writer.contains_key(&self.id) {
                debug_assert!(writer.get(&self.id).unwrap() != &ToFlushResourceAction::Remove);
                writer.remove(&self.id);
            } else {
                writer.insert(self.id, ToFlushResourceAction::Remove);
            }
        }
        {
            let mut writter = store_ref.weak_map.write().unwrap();
            writter.remove(&self.id);
        }
    }
}

impl MistyResourceManager {
    pub fn new() -> Self {
        Self {
            store: Arc::new(MistyResourceManagerStore {
                pending_actions: Arc::new(RwLock::new(Some(Default::default()))),
                weak_map: Default::default(),
            }),
        }
    }

    pub fn get_handle(&self, id: MistyResourceId) -> Option<MistyResourceHandle> {
        let reader = self.store.weak_map.read().unwrap();
        reader
            .get(&id)
            .map(|ptr| ptr.upgrade())
            .unwrap_or_default()
            .map(|ptr| MistyResourceHandle { ptr })
    }

    pub fn insert(&self, buf: impl Into<Vec<u8>> + 'static) -> MistyResourceHandle {
        let id = MistyResourceId::alloc();
        let buf: Vec<u8> = buf.into();

        let ptr = Arc::new(MistyResourceHandleInner {
            id,
            store_ref: Arc::downgrade(&self.store),
            buf,
        });
        let handle = MistyResourceHandle { ptr: ptr.clone() };

        {
            let mut writter = self.store.pending_actions.write().unwrap();
            let writer = writter.as_mut().unwrap();
            writer.insert(id, ToFlushResourceAction::Insert(handle.clone()));
        }
        {
            let mut writter = self.store.weak_map.write().unwrap();
            writter.insert(id, Arc::downgrade(&ptr));
        }

        handle
    }

    pub(crate) fn take_all_actions(&self) -> Vec<ResourceUpdateAction> {
        let pending_ids = {
            let mut writer = self.store.pending_actions.write().unwrap();
            let cloned = writer.take();
            *writer = Some(Default::default());
            cloned.unwrap()
        };

        let mut ret: Vec<ResourceUpdateAction> = Default::default();
        for (id, action) in pending_ids {
            match action {
                ToFlushResourceAction::Insert(handle) => {
                    ret.push(ResourceUpdateAction::Insert(id, handle.load().clone()));
                }
                ToFlushResourceAction::Remove => {
                    ret.push(ResourceUpdateAction::Remove(id));
                }
            }
        }
        ret
    }
}
