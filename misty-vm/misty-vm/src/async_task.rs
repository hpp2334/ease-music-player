use std::{
    cell::RefCell,
    collections::HashMap,
    hash::Hash,
    ops::DerefMut,
    rc::Rc,
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};

use async_task::Task;
use misty_async::{BoxFuture, IAsyncRuntimeAdapter};

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub struct AsyncTaskId(u64);

#[derive(Default, Clone)]
pub struct AsyncTasks {
    tasks: Rc<RefCell<HashMap<AsyncTaskId, Task<()>>>>,
    id_allocator: Arc<AtomicU64>,
}

#[derive(Default, Clone)]
pub struct AsyncTaskPod {
    id: Rc<RefCell<Option<AsyncTaskId>>>,
}

impl AsyncTasks {
    pub(crate) fn allocate(&self) -> AsyncTaskId {
        let id = self
            .id_allocator
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        AsyncTaskId(id)
    }

    pub(crate) fn bind(&self, id: AsyncTaskId, task: Task<()>) {
        let old = self.tasks.borrow_mut().insert(id, task);
        assert!(old.is_none());
    }

    pub fn cancel_all(&self) {
        std::mem::swap(self.tasks.borrow_mut().deref_mut(), &mut HashMap::new());
    }

    pub fn cancel(&self, id: AsyncTaskId) {
        self.tasks.borrow_mut().remove(&id);
    }

    pub(crate) fn remove(&self, id: AsyncTaskId) {
        self.tasks.borrow_mut().remove(&id);
    }
}

impl AsyncTaskPod {
    pub fn cancel(&self, tasks: &AsyncTasks) {
        let mut id = self.id.borrow_mut();
        {
            if let Some(id) = id.clone() {
                tasks.cancel(id);
            }
        }
        *id = None;
    }

    pub(crate) fn set(&self, id: AsyncTaskId) {
        assert!(self.id.borrow().is_none());
        *self.id.borrow_mut() = Some(id);
    }
}

pub(crate) struct DefaultAsyncRuntimeAdapter;

impl IAsyncRuntimeAdapter for DefaultAsyncRuntimeAdapter {
    fn is_main_thread(&self) -> bool {
        todo!()
    }

    fn on_spawn_locals(&self) {
        todo!()
    }

    fn sleep(&self, _duration: Duration) -> BoxFuture<()> {
        todo!()
    }

    fn get_time(&self) -> Duration {
        todo!()
    }
}
