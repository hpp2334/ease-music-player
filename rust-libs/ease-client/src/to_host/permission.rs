use misty_vm::misty_to_host;

pub trait IPermissionService: Send + Sync + 'static {
    fn have_storage_permission(&self) -> bool;
    fn request_storage_permission(&self);
}
misty_to_host!(PermissionService, IPermissionService);
