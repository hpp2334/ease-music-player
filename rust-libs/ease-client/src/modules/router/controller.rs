use misty_vm::controllers::MistyControllerContext;

use crate::modules::error::{EaseResult, EASE_RESULT_NIL};

use super::{service::*, typ::*};

pub fn controller_update_root_subkey(
    ctx: MistyControllerContext,
    arg: RootRouteSubKey,
) -> EaseResult<()> {
    update_root_subkey(ctx.handle(), arg);
    EASE_RESULT_NIL
}
