use misty_vm::misty_to_host;

#[uniffi::export(with_foreign)]
pub trait IRouter: Send + Sync + 'static {
    fn route_storage(&self);
}

misty_to_host!(RouterService, IRouter);
