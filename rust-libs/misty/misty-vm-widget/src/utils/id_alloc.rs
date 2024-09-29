use std::sync::{atomic::{AtomicU64, AtomicUsize}, Arc};

#[derive(Clone)]
pub struct IdAlloc {
    id: Arc<AtomicU64>
}

impl IdAlloc {
    pub fn new() -> Self {
        Self { id: Arc::new(AtomicU64::new(1)) }
    }

    pub fn allocate(&self) -> u64 {
        self.id.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}