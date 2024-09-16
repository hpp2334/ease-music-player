use ease_client_shared::backends::app::ArgInitializeApp;
use misty_vm::controllers::MistyControllerContext;

use crate::modules::error::EaseResult;

use super::service::init_backend;

pub fn controller_init_app_internal(
    ctx: MistyControllerContext,
    arg: ArgInitializeApp,
) -> EaseResult<()> {
    init_backend(ctx.handle(), arg);
    Ok(())
}
