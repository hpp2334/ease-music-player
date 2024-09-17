use std::{any::Any, sync::Arc};

use crate::{
    client::{MistyClientHandle, MistyClientInner},
    resources::ResourceUpdateAction,
    states::GuardCleanupStatesForPanic,
};

pub struct MistyControllerContext<'a> {
    pub(crate) handle: MistyClientHandle<'a>,
}

impl<'a> MistyControllerContext<'a> {
    pub(crate) fn new(handle: MistyClientHandle<'a>) -> Self {
        Self { handle }
    }

    pub fn handle(&self) -> MistyClientHandle {
        self.handle
    }
}

pub trait MistyController<Arg, E> {
    fn call(&self, ctx: MistyControllerContext, arg: Arg) -> Result<(), E>;
}

impl<Arg, E, F> MistyController<Arg, E> for F
where
    F: Fn(MistyControllerContext, Arg) -> Result<(), E>,
{
    fn call(&self, ctx: MistyControllerContext, arg: Arg) -> Result<(), E> {
        self(ctx, arg)
    }
}

pub struct ControllerRet<R> {
    pub changed_view: Option<R>,
    pub changed_resources: Vec<ResourceUpdateAction>,
}

pub(crate) fn call_controller<R, Controller, Arg, E>(
    inner: &Arc<MistyClientInner>,
    controller: Controller,
    arg: Arg,
) -> Result<ControllerRet<R>, E>
where
    R: Any + Default + Send + Sync + 'static,
    Controller: MistyController<Arg, E>,
{
    let controller_name = std::any::type_name::<Controller>();
    let span = tracing::span!(tracing::Level::DEBUG, "call controller", controller_name);
    let _span_guard = span.enter();

    let mut _cleanup_guard = GuardCleanupStatesForPanic::new(Arc::downgrade(inner));

    let ctx = MistyControllerContext::new(MistyClientHandle { inner: &inner });
    inner.state_manager.enter_mut_span();
    let res = controller.call(ctx, arg);
    let can_notify = inner.state_manager.leave_mut_span();

    let mut changed_view: Option<R> = None;
    let mut changed_actions: Vec<ResourceUpdateAction> = Default::default();

    if can_notify {
        changed_actions = inner.resource_manager.take_all_actions();
        changed_view = Some(inner.view_manager.build_view(&inner).cast::<R>());
        inner.state_manager.clear_updated_states();
    }

    _cleanup_guard.mark();

    if let Err(e) = res {
        return Err(e);
    }
    Ok(ControllerRet {
        changed_view,
        changed_resources: changed_actions,
    })
}
