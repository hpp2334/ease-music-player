use misty_vm::misty_service;

pub trait IToastService: Send + Sync + 'static {
    fn error(&self, msg: String);
}
misty_service!(ToastService, IToastService);
