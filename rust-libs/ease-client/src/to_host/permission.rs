use ease_client_shared::backends::music::{Music, MusicId};
use misty_vm::misty_to_host;

#[uniffi::export(with_foreign)]
pub trait IPermissionService: Send + Sync + 'static {
    fn have_storage_permission(&self) -> bool;
    fn request_storage_permission(&self);
}
misty_to_host!(PermissionService, IPermissionService);
