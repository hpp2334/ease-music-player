use ease_client_shared::uis::to_hosts::IToastService;
use misty_vm::misty_service;

misty_service!(ToastService, IToastService);

pub struct ToHostToastService;
impl IToastService for ToHostToastService {
    fn error(&self, msg: String) {
        todo!()
    }
}
