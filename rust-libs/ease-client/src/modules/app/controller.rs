use misty_vm::controllers::MistyControllerContext;

use crate::modules::error::EaseResult;

pub use super::service::ArgInitializeApp;
use super::service::{app_boostrap, update_storage_permission};

pub fn controller_initialize_app(
    ctx: MistyControllerContext,
    arg: ArgInitializeApp,
) -> EaseResult<()> {
    app_boostrap(ctx.handle(), arg)
}

pub fn controller_update_storage_permission(
    ctx: MistyControllerContext,
    arg: bool,
) -> EaseResult<()> {
    update_storage_permission(ctx.handle(), arg);
    Ok(())
}
