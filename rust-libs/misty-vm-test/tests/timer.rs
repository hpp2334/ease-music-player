use std::{convert::Infallible, time::Duration};

use misty_vm::{
    async_task::MistyAsyncTaskTrait, controllers::MistyControllerContext, misty_service, services::MistyServiceTrait, states::MistyStateTrait, MistyAsyncTask, MistyState
};

#[derive(Debug, Default, Clone, MistyState)]
struct GlobalState {
    pub host_time: i64,
}

#[derive(Debug, Default, Clone)]
struct RootViewModelState {
    pub host_time: i64,
}

pub trait ITimerService: Send + Sync + 'static {
    fn request_get_host_time(&self);
}

misty_service!(TimerService, ITimerService);

#[derive(Debug, MistyAsyncTask)]
struct SpawnGetHostTimeIntervalAsyncTask;

fn controller_initialize_app(ctx: MistyControllerContext, _arg: ()) -> Result<(), Infallible> {
    SpawnGetHostTimeIntervalAsyncTask::spawn_once(&ctx, |ctx| async move {
        loop {
            TimerService::of_async(&ctx).request_get_host_time();

            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        #[allow(unreachable_code)]
        Result::<(), Infallible>::Ok(())
    });
    Ok(())
}

fn controller_set_host_time(ctx: MistyControllerContext, arg: i64) -> Result<(), Infallible> {
    GlobalState::update(&ctx, |state| {
        state.host_time = arg;
    });
    Ok(())
}

fn timer_view_model(states: &GlobalState, root: &mut RootViewModelState) {
    root.host_time = states.host_time;
}

#[cfg(test)]
mod test {
    use std::{
        sync::{atomic::AtomicI64, Arc},
        time::Duration,
    };

    use misty_vm::{
        misty_states, services::MistyServiceManager, states::MistyStateManager,
        views::MistyViewModelManager,
    };
    use misty_vm_test::{TestApp, TestAppContainer};

    use crate::{
        controller_initialize_app, controller_set_host_time, timer_view_model, GlobalState,
        ITimerService, RootViewModelState, TimerService,
    };

    struct FakeTimer {
        count: AtomicI64,
    }
    impl FakeTimer {
        pub fn new() -> Arc<Self> {
            Arc::new(Self {
                count: Default::default(),
            })
        }
        pub fn now(&self) -> i64 {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        }
    }

    struct FakeTimerService {
        timer: Arc<FakeTimer>,
        app_container: TestAppContainer<RootViewModelState>,
    }
    impl ITimerService for FakeTimerService {
        fn request_get_host_time(&self) {
            self.app_container
                .call_controller(controller_set_host_time, self.timer.now());
        }
    }

    fn build_app() -> TestApp<RootViewModelState> {
        let app_container = TestAppContainer::new(|changed, state| {
            *state = changed;
        });
        let fake_timer = FakeTimer::new();

        let view_manager = MistyViewModelManager::builder()
            .register(timer_view_model)
            .build();
        let service_manager = MistyServiceManager::builder()
            .add(TimerService::new(FakeTimerService {
                timer: fake_timer,
                app_container: app_container.clone(),
            }))
            .build();
        let state_manager = MistyStateManager::new(misty_states!(GlobalState));

        let app = TestApp::new(view_manager, service_manager, state_manager, app_container);
        app.app().call_controller(controller_initialize_app, ());
        app
    }

    #[tokio::test]
    async fn test_main() {
        let app = build_app();

        tokio::time::sleep(Duration::from_millis(2500)).await;

        let state = app.state();
        assert_eq!(state.host_time, 2);
    }
}
