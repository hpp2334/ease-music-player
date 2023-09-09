use crate::modules::error::EaseResult;
use crate::modules::error::EASE_RESULT_NIL;

use super::service::*;
use super::typ::*;
use misty_vm::controllers::MistyControllerContext;

pub fn controller_remove_storage(ctx: MistyControllerContext, id: StorageId) -> EaseResult<()> {
    remove_storage(ctx.handle(), id)?;
    EASE_RESULT_NIL
}

pub fn controller_locate_entry(ctx: MistyControllerContext, path: String) -> EaseResult<()> {
    locate_entry(ctx.handle(), path)?;
    EASE_RESULT_NIL
}

pub fn controller_select_entry(ctx: MistyControllerContext, path: String) -> EaseResult<()> {
    select_entry(ctx.handle(), path);
    EASE_RESULT_NIL
}

pub fn controller_toggle_all_checked_entries(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    toggle_all_checked_entries(ctx.handle());
    EASE_RESULT_NIL
}

pub fn controller_select_storage_in_import(
    ctx: MistyControllerContext,
    id: StorageId,
) -> EaseResult<()> {
    select_storage_in_import(ctx.handle(), id)?;
    EASE_RESULT_NIL
}

pub fn controller_refresh_current_storage_in_import(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    refresh_current_storage_in_import(ctx.handle())?;
    EASE_RESULT_NIL
}

pub fn controller_finish_selected_entries_in_import(
    ctx: MistyControllerContext,
    _arg: (),
) -> EaseResult<()> {
    finish_select_entries_in_import(ctx.handle())?;
    EASE_RESULT_NIL
}

// Edit Storage

pub fn controller_prepare_edit_storage(
    ctx: MistyControllerContext,
    id: Option<StorageId>,
) -> EaseResult<()> {
    prepare_edit_storage(ctx.handle(), id)?;
    EASE_RESULT_NIL
}

pub fn controller_upsert_storage(
    ctx: MistyControllerContext,
    arg: ArgUpsertStorage,
) -> EaseResult<()> {
    upsert_storage(ctx.handle(), arg)?;
    EASE_RESULT_NIL
}

pub fn controller_test_connection(
    ctx: MistyControllerContext,
    arg: ArgUpsertStorage,
) -> EaseResult<()> {
    edit_storage_test_connection(ctx.handle(), arg)?;
    EASE_RESULT_NIL
}
