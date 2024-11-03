#[uniffi::export(with_foreign)]
pub trait IFlushNotifier: Send + Sync + 'static {
    fn handle_notify(&self);
}

