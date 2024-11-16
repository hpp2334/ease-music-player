use misty_vm::misty_to_host;

pub trait IToastService: Send + Sync + 'static {
    fn error(&self, msg: String);
}

misty_to_host!(ToastService, IToastService);
