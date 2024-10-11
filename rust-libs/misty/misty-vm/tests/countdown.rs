use std::{convert::Infallible, default, time::Duration};

use misty_vm::{App, AppBuilderContext, IAsyncRuntimeAdapter, Model, ViewModel, ViewModelContext};

enum Event {
    Start,
    Pause,
    Stop,
    Update { value: u32 },
    Tick,
}

#[derive(Default, PartialEq, Eq)]
enum PlayingState {
    #[default]
    Pending,
    Playing,
    Pause,
}

#[derive(Default)]
struct CountdownState {
    pub value: u32,
    pub state: PlayingState,
}

struct CountdownVM {
    value: Model<CountdownState>,
}

impl CountdownVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self { value: cx.model() }
    }
}

impl ViewModel<Event, Infallible> for CountdownVM {
    fn on_event(&self, cx: &ViewModelContext, e: &Event) -> Result<(), Infallible> {
        match e {
            Event::Start => {
                {
                    let mut state = cx.model_mut(&self.value);
                    state.state = PlayingState::Playing;
                }
                cx.enqueue_emit(Event::Tick);
            }
            Event::Pause => {
                let mut state = cx.model_mut(&self.value);
                state.state = PlayingState::Pause;
            }
            Event::Stop => {
                let playing = {
                    let value = cx.model_get(&self.value);
                    value.state == PlayingState::Playing
                };
                assert_eq!(playing, true);

                {
                    let mut state = cx.model_mut(&self.value);
                    state.state = PlayingState::Pending;
                    state.value = 0;
                }
            }
            Event::Update { value } => {
                let playing = {
                    let value = cx.model_get(&self.value);
                    value.state == PlayingState::Playing
                };
                assert_eq!(playing, false);

                {
                    let mut state = cx.model_mut(&self.value);
                    state.value = *value;
                }
            }
            Event::Tick => {
                let model = self.value.clone();
                cx.spawn(|cx| async move {
                    tokio::time::sleep(Duration::from_secs(1)).await;

                    let emit_next_tick = {
                        let mut state = cx.model_mut(&model);
                        if state.value > 0 {
                            state.value -= 1;
                            true
                        } else {
                            state.state = PlayingState::Pending;
                            false
                        }
                    };
                    if emit_next_tick {
                        cx.enqueue_emit(Event::Tick);
                    }
                });
            }
        }

        Ok(())
    }
}

fn build_app(adapter: impl IAsyncRuntimeAdapter) -> App {
    App::builder()
        .with_view_models(|cx, builder| {
            builder.add(CountdownVM::new(cx));
        })
        .with_async_runtime_adapter(adapter)
        .build()
}

#[cfg(test)]
mod tests {
    use std::sync::{atomic::AtomicU64, Arc};

    use misty_vm::IAsyncRuntimeAdapter;

    use crate::{build_app, CountdownState, Event};

    struct AsyncRuntimeInternal {
        runtime: tokio::runtime::Runtime,
        local: tokio::task::LocalSet,
        id_alloc: AtomicU64,
    }

    #[derive(Clone)]
    struct AsyncRuntime {
        store: Arc<AsyncRuntimeInternal>,
    }
    impl AsyncRuntime {
        pub fn new() -> Self {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            let local = tokio::task::LocalSet::new();

            Self {
                store: Arc::new(AsyncRuntimeInternal {
                    runtime: rt,
                    local,
                    id_alloc: AtomicU64::new(0),
                }),
            }
        }
        pub fn enter(&self) -> tokio::runtime::EnterGuard<'_> {
            self.store.runtime.enter()
        }
    }
    impl IAsyncRuntimeAdapter for AsyncRuntime {
        fn spawn_local(&self, future: misty_vm::LocalBoxFuture<'static, ()>) -> u64 {
            self.store.local.spawn_local(future);
            let id = self
                .store
                .id_alloc
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            id
        }
    }

    #[test]
    fn test_update_and_start() {
        let rt = AsyncRuntime::new();
        let _guard = rt.enter();

        let app = build_app(rt.clone());
        app.emit(Event::Update { value: 10 });
        app.emit(Event::Start);

        {
            let v = app.model::<CountdownState>();
            assert_eq!(v.value, 1);
        }
    }
}
