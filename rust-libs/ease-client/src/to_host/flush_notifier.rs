#[uniffi::export(with_foreign)]
pub trait IFlushNotifier: Send + Sync + 'static {
    fn notify(&self);
}

