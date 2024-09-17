use std::{collections::HashMap, convert::Infallible};

use misty_vm::client::AsMistyClientHandle;
use misty_vm::services::MistyServiceTrait;
use misty_vm::states::MistyStateTrait;
use misty_vm::{
    async_task::MistyAsyncTaskTrait, controllers::MistyControllerContext, misty_service,
};
use misty_vm::{MistyAsyncTask, MistyState};
use rand::{
    distributions::{Distribution, Uniform},
    Rng,
};

#[derive(Debug, Default, Clone, MistyState)]
struct GlobalState {
    pub store: HashMap<i32, bool>,
    pub done: bool,
}

#[derive(Debug, Default, Clone)]
struct RootViewModelState {
    pub store: HashMap<i32, bool>,
    pub done: bool,
}

#[derive(Debug, MistyAsyncTask)]
struct OneAsyncTask;

#[derive(Debug, MistyAsyncTask)]
struct WaitAllAsyncTask;

pub trait IAService: Send + Sync + 'static {
    fn to_host(&self, id: i32, typ: FinalUpdate);
}

misty_service!(AService, IAService);

#[derive(Debug, Clone, Copy)]
pub enum UpdateType {
    Update,
    Panic,
    Async,
    AsyncPanic,
    AsyncSchedulePanic,
    NestedRandomFinalUpdate,
    NestedRandomFinalPanic,
    AsyncNestedRandomFinalUpdate,
    AsyncNestedRandomFinalPanic,
}

#[derive(Debug, Clone, Copy)]
pub enum FinalUpdate {
    UpdateLike,
    PanicLike,
}

pub fn generate_update_type<R>(rng: &mut R, final_update: Option<FinalUpdate>) -> UpdateType
where
    R: Rng + ?Sized,
{
    let uniform: Uniform<i32> = Uniform::new(0, 10);

    if let Some(final_update) = final_update {
        match final_update {
            FinalUpdate::UpdateLike => match uniform.sample(rng) {
                0 => UpdateType::Async,
                1 => UpdateType::NestedRandomFinalUpdate,
                2 => UpdateType::AsyncNestedRandomFinalUpdate,
                _ => UpdateType::Update,
            },
            FinalUpdate::PanicLike => match uniform.sample(rng) {
                0 => UpdateType::AsyncPanic,
                1 => UpdateType::AsyncSchedulePanic,
                2 => UpdateType::NestedRandomFinalPanic,
                3 => UpdateType::AsyncNestedRandomFinalPanic,
                _ => UpdateType::Panic,
            },
            #[allow(unreachable_patterns)]
            _ => unreachable!(),
        }
    } else {
        match uniform.sample(rng) {
            0 => UpdateType::Panic,
            1 => UpdateType::Async,
            2 => UpdateType::AsyncPanic,
            3 => UpdateType::AsyncSchedulePanic,
            4 => UpdateType::NestedRandomFinalUpdate,
            5 => UpdateType::NestedRandomFinalPanic,
            6 => UpdateType::AsyncNestedRandomFinalUpdate,
            7 => UpdateType::AsyncNestedRandomFinalPanic,
            _ => UpdateType::Update,
        }
    }
}

fn update_state<'a>(ctx: impl AsMistyClientHandle<'a>, id: i32) {
    GlobalState::update(ctx, |state| {
        let entry = state.store.entry(id);
        let entry = entry.or_default();
        *entry = true;
    });
}

fn controller_update(
    ctx: MistyControllerContext,
    (id, typ): (i32, UpdateType),
) -> Result<(), Infallible> {
    match typ {
        UpdateType::Update => {
            update_state(&ctx, id);
        }
        UpdateType::Panic => {
            panic!("generate panic!\n");
        }
        UpdateType::Async => {
            OneAsyncTask::spawn(&ctx, move |ctx| async move {
                ctx.schedule(move |ctx| {
                    update_state(ctx, id);
                    Result::<(), Infallible>::Ok(())
                });
                Result::<(), Infallible>::Ok(())
            });
        }
        UpdateType::AsyncPanic => {
            OneAsyncTask::spawn(&ctx, |_| async {
                panic!("generate panic in async!\n");
                #[allow(unreachable_code)]
                Result::<(), Infallible>::Ok(())
            });
        }
        UpdateType::AsyncSchedulePanic => {
            OneAsyncTask::spawn(&ctx, |ctx| async move {
                ctx.schedule(|_| -> Result<(), Infallible> {
                    panic!("generate panic in async schedule!\n");
                });
                Result::<(), Infallible>::Ok(())
            });
        }
        UpdateType::NestedRandomFinalUpdate => {
            AService::of(&ctx).to_host(id, FinalUpdate::UpdateLike);
        }
        UpdateType::NestedRandomFinalPanic => {
            AService::of(&ctx).to_host(id, FinalUpdate::PanicLike);
        }
        UpdateType::AsyncNestedRandomFinalUpdate => {
            OneAsyncTask::spawn(&ctx, move |ctx| async move {
                AService::of_async(&ctx).to_host(id, FinalUpdate::UpdateLike);
                Result::<(), Infallible>::Ok(())
            });
        }
        UpdateType::AsyncNestedRandomFinalPanic => {
            OneAsyncTask::spawn(&ctx, move |ctx| async move {
                AService::of_async(&ctx).to_host(id, FinalUpdate::PanicLike);
                Result::<(), Infallible>::Ok(())
            });
        }
    }
    Ok(())
}

fn controller_wait_all(ctx: MistyControllerContext, _arg: ()) -> Result<(), Infallible> {
    WaitAllAsyncTask::spawn_once(&ctx, |ctx| async move {
        ctx.schedule(|ctx| -> Result<(), Infallible> {
            GlobalState::update(ctx, |state| {
                state.done = true;
            });
            Result::<(), Infallible>::Ok(())
        });
        Result::<(), Infallible>::Ok(())
    });
    Result::<(), Infallible>::Ok(())
}

fn global_view_model(states: &GlobalState, root: &mut RootViewModelState) {
    root.store = states.store.clone();
    root.done = states.done;
}

#[cfg(test)]
mod test {
    use std::{
        ops::DerefMut,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use crate::{
        controller_update, controller_wait_all, generate_update_type, global_view_model, AService,
        FinalUpdate, GlobalState, IAService, RootViewModelState, UpdateType,
    };
    use misty_vm::{
        misty_states, services::MistyServiceManager, states::MistyStateManager,
        views::MistyViewModelManager,
    };
    use misty_vm_test::{TestApp, TestAppContainer};
    use rand::{rngs::StdRng, SeedableRng};
    use tokio::task::JoinSet;

    fn build_app() -> TestApp<RootViewModelState> {
        let app_container = TestAppContainer::<RootViewModelState>::new(|changed, state| {
            for (k, v) in changed.store {
                state.store.insert(k, v);
            }
            state.done = changed.done;
        });

        let view_manager = MistyViewModelManager::builder()
            .register(global_view_model)
            .build();
        let service_manager = MistyServiceManager::builder()
            .add(AService::new(FakeAServiceImpl {
                app_container: app_container.clone(),
                rng: Arc::new(Mutex::new(StdRng::seed_from_u64(1))),
            }))
            .build();
        let state_manager = MistyStateManager::new(misty_states!(GlobalState));

        let app = TestApp::new(view_manager, service_manager, state_manager, app_container);
        app
    }

    async fn wait_done(app: &TestApp<RootViewModelState>) -> bool {
        for _ in 0..50 {
            let state = app.state();
            if state.done {
                return true;
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        return false;
    }

    struct FakeAServiceImpl {
        app_container: TestAppContainer<RootViewModelState>,
        rng: Arc<Mutex<StdRng>>,
    }
    impl IAService for FakeAServiceImpl {
        fn to_host(&self, id: i32, final_update: FinalUpdate) {
            let next_typ = {
                let mut rng = self.rng.lock().unwrap();
                generate_update_type(rng.deref_mut(), Some(final_update))
            };
            self.app_container
                .call_controller(controller_update, (id, next_typ));
        }
    }

    struct InputOperations {
        ops: Vec<(UpdateType, i32)>,
    }
    fn generate_input_operations(n: i32, final_update: Option<FinalUpdate>) -> InputOperations {
        let mut ret = InputOperations {
            ops: Default::default(),
        };
        let mut rng = StdRng::seed_from_u64(0);

        for id in 0..n {
            let typ = generate_update_type(&mut rng, final_update);
            ret.ops.push((typ, id));
        }

        ret
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_with_panic() {
        // ignore backtrace detail here, because we generate panic on purpose.
        std::env::set_var("RUST_BACKTRACE", "0");

        let subscriber = tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);

        let test_app = build_app();

        const N: i32 = 10000;
        let input = generate_input_operations(N, None);

        let mut join_set = JoinSet::new();

        for (op, id) in input.ops.clone().into_iter() {
            let app = test_app.app();
            join_set.spawn(async move {
                app.call_controller(controller_update, (id, op));
            });
        }

        while let Some(_) = join_set.join_next().await {}
        {
            let app = test_app.app();
            join_set.spawn(async move {
                app.call_controller(controller_wait_all, ());
            });
        }
        while let Some(_) = join_set.join_next().await {}
        let done = wait_done(&test_app).await;
        assert_eq!(done, true);

        let state = test_app.state();
        for (op, id) in input.ops.into_iter() {
            let current = state.store.get(&id).map(|v| *v).unwrap_or_default();
            match op {
                UpdateType::Update => {
                    if !current {
                        panic!("expect True but current is False, op {:?}, id {}", op, id);
                    }
                }
                UpdateType::Panic
                | UpdateType::AsyncPanic
                | UpdateType::AsyncSchedulePanic
                | UpdateType::NestedRandomFinalPanic
                | UpdateType::AsyncNestedRandomFinalPanic => {
                    if current {
                        panic!("expect False but current is True, op {:?}, id {}", op, id);
                    }
                }
                _ => {}
            }
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_without_panic() {
        let subscriber = tracing_subscriber::FmtSubscriber::builder()
            .with_max_level(tracing::Level::DEBUG)
            .finish();
        let _ = tracing::subscriber::set_global_default(subscriber);
        let test_app = build_app();

        const N: i32 = 10000;
        let input = generate_input_operations(N, Some(FinalUpdate::UpdateLike));

        let mut join_set = JoinSet::new();

        for (op, id) in input.ops.clone().into_iter() {
            let app = test_app.app();
            join_set.spawn(async move {
                let _ = app.call_controller(controller_update, (id, op));
            });
        }

        while let Some(_) = join_set.join_next().await {}

        {
            let app = test_app.app();
            join_set.spawn(async move {
                let _ = app.call_controller(controller_wait_all, ());
            });
        }
        while let Some(_) = join_set.join_next().await {}
        let done = wait_done(&test_app).await;
        assert_eq!(done, true);

        let state = test_app.state();
        for (_, id) in input.ops.into_iter() {
            assert_eq!(state.store.get(&id), Some(&true));
        }
    }
}
