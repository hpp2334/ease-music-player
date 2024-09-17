use std::{
    convert::Infallible,
    sync::{Arc, RwLock},
};

use crate::{
    client::MistyClientHandle,
    controllers::MistyControllerContext,
    signals::{MistySignal, SignalEmitter},
};

pub(crate) struct ScheduleManager {
    tasks: Arc<RwLock<Vec<ScheduledTask>>>,
}

pub(crate) struct ScheduledTask {
    handler: Box<dyn FnOnce(MistyClientHandle) + Send + Sync>,
}

impl ScheduledTask {
    fn new<E>(
        handler: impl FnOnce(MistyClientHandle) -> Result<(), E> + Send + Sync + 'static,
    ) -> Self
    where
        E: std::fmt::Display,
    {
        Self {
            handler: Box::new(|handle| {
                let err = handler(handle);
                if let Err(err) = err {
                    tracing::error!("schedule fail, error: {}", err);
                }
            }),
        }
    }

    fn run(self, handle: MistyClientHandle) {
        (self.handler)(handle);
    }
}

impl ScheduleManager {
    pub fn new() -> Self {
        Self {
            tasks: Default::default(),
        }
    }

    pub fn enqueue<E>(
        &self,
        signal_emitter: &SignalEmitter,
        handler: impl FnOnce(MistyClientHandle) -> Result<(), E> + Send + Sync + 'static,
    ) where
        E: std::fmt::Display,
    {
        {
            let mut tasks = self.tasks.write().unwrap();
            tasks.push(ScheduledTask::new(handler));
        }
        signal_emitter.emit(MistySignal::Schedule);
    }

    pub fn take_all_tasks(&self) -> Vec<ScheduledTask> {
        let mut current_tasks = vec![];
        {
            let mut tasks = self.tasks.write().unwrap();
            std::mem::swap::<Vec<ScheduledTask>>(&mut *tasks, current_tasks.as_mut());
        }
        current_tasks
    }
}

pub(crate) fn controller_flush_scheduled_tasks(
    ctx: MistyControllerContext,
    _arg: (),
) -> Result<(), Infallible> {
    let handle = ctx.handle();
    let tasks = handle.inner.schedule_manager.take_all_tasks();

    for task in tasks.into_iter() {
        task.run(handle.clone());
    }

    Ok(())
}
