

#[uniffi::export(with_foreign)]
pub trait IFlushSignal: Send + Sync + 'static {
    fn flush(&self);
}
