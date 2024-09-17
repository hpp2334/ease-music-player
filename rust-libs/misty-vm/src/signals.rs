use std::sync::{Arc, RwLock};

pub enum MistySignal {
    Schedule,
}

pub struct SignalEmitter {
    handler: Arc<RwLock<Arc<dyn Fn(MistySignal) + Send + Sync + 'static>>>,
}

impl SignalEmitter {
    pub fn new() -> Self {
        Self {
            handler: Arc::new(RwLock::new(Arc::new(|_| {
                tracing::warn!("singal emiiter is not binding any handler");
            }))),
        }
    }

    pub fn set(&self, f: impl Fn(MistySignal) + Send + Sync + 'static) {
        let mut w = self.handler.write().unwrap();
        *w = Arc::new(f);
    }

    pub(crate) fn emit(&self, singal: MistySignal) {
        let f = self.handler.read().unwrap().clone();
        f(singal);
    }
}
