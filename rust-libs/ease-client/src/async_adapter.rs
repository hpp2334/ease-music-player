use std::{sync::Arc, time::Duration};

use misty_vm::{IAsyncRuntimeAdapter, LocalBoxFuture};

use crate::to_host::flush_notifier::IFlushNotifier;

pub(crate) struct AsyncAdapter {
    notifier: Arc<dyn IFlushNotifier>,
}

impl AsyncAdapter {
    pub fn new(notifier: Arc<dyn IFlushNotifier>) -> Self {
        Self { notifier }
    }
}

impl IAsyncRuntimeAdapter for AsyncAdapter {
    fn on_schedule(&self) {
        self.notifier.handle_notify();
    }

    fn sleep(&self, duration: Duration) -> LocalBoxFuture<'static, ()> {
        Box::pin(tokio::time::sleep(duration))
    }

    fn get_time(&self) -> Duration {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
    }
}
