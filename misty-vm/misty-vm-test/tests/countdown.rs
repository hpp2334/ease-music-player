use std::{convert::Infallible, default, time::Duration};

use misty_vm::{App, AppBuilderContext, IAsyncRuntimeAdapter, Model, ViewModel, ViewModelContext};

#[derive(Debug)]
enum Event {
    Start,
    Pause,
    Stop,
    Update { value: u32 },
    Tick { session: u32 },
}

#[derive(Debug, Default, PartialEq, Eq)]
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
    pub ticking_session: u32,
}

struct CountdownVM {
    value: Model<CountdownState>,
}

impl CountdownVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self { value: cx.model() }
    }

    fn tick(&self, cx: &ViewModelContext, session: u32) {
        let model = self.value.clone();
        {
            let mut state = cx.model_mut(&model);
            if state.state != PlayingState::Playing {
                return;
            }
            if state.ticking_session != session {
                return;
            }
            if state.value > 0 {
                state.value -= 1;
            }
            if state.value == 0 {
                state.state = PlayingState::Pending;
            }
        }
        self.schedule_next_tick(cx);
    }

    fn schedule_next_tick(&self, cx: &ViewModelContext) {
        let model = self.value.clone();
        let emit_next_tick = {
            let state = cx.model_get(&model);
            state.state == PlayingState::Playing && state.value > 0
        };
        if emit_next_tick {
            let session = cx.model_get(&model).ticking_session;
            cx.spawn(|cx| async move {
                cx.sleep(Duration::from_secs(1)).await;
                cx.enqueue_emit(Event::Tick { session });
            });
        }
    }

    fn assert_state(&self, cx: &ViewModelContext, target: PlayingState) {
        let matching = {
            let value = cx.model_get(&self.value);
            value.state == target
        };
        assert_eq!(matching, true);
    }
}

impl ViewModel<Event, Infallible> for CountdownVM {
    fn on_event(&self, cx: &ViewModelContext, e: &Event) -> Result<(), Infallible> {
        match e {
            Event::Start => {
                {
                    let mut state = cx.model_mut(&self.value);
                    assert!(
                        state.state == PlayingState::Pause || state.state == PlayingState::Pending
                    );
                    state.state = PlayingState::Playing;
                    state.ticking_session += 1;
                }
                self.schedule_next_tick(cx);
            }
            Event::Pause => {
                self.assert_state(cx, PlayingState::Playing);

                let mut state = cx.model_mut(&self.value);
                state.state = PlayingState::Pause;
                state.ticking_session += 1;
            }
            Event::Stop => {
                self.assert_state(cx, PlayingState::Playing);

                {
                    let mut state = cx.model_mut(&self.value);
                    state.state = PlayingState::Pending;
                    state.value = 0;
                    state.ticking_session += 1;
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
            Event::Tick { session } => {
                self.tick(cx, *session);
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

    use std::time::Duration;

    use misty_vm_test::AsyncRuntime;

    use crate::{build_app, CountdownState, Event, PlayingState};

    #[test]
    fn test_update_and_start() {
        let rt = AsyncRuntime::new();
        let _guard = rt.enter();

        let app = build_app(rt.clone());
        app.emit(Event::Update { value: 10 });
        app.emit(Event::Start);
        rt.advance(Duration::from_secs(9));

        {
            let v = app.model::<CountdownState>();
            assert_eq!(v.value, 1);
        }
    }

    #[test]
    fn test_update_start_and_pause() {
        let rt = AsyncRuntime::new();
        let _guard = rt.enter();

        let app = build_app(rt.clone());
        app.emit(Event::Update { value: 5 });
        app.emit(Event::Start);
        rt.advance(Duration::from_secs(2));

        app.emit(Event::Pause);
        rt.advance(Duration::from_secs(2));

        {
            let v = app.model::<CountdownState>();
            assert_eq!(v.value, 3);
            assert_eq!(v.state, PlayingState::Pause);
        }

        app.emit(Event::Start);
        rt.advance(Duration::from_secs(2));

        {
            let v = app.model::<CountdownState>();
            assert_eq!(v.value, 1);
            assert_eq!(v.state, PlayingState::Playing);
        }
    }

    #[test]
    fn test_update_and_restart() {
        let rt = AsyncRuntime::new();
        let _guard = rt.enter();

        let app = build_app(rt.clone());
        app.emit(Event::Update { value: 5 });
        app.emit(Event::Start);
        rt.advance(Duration::from_secs(3));

        app.emit(Event::Stop);
        app.emit(Event::Update { value: 8 });
        {
            let v = app.model::<CountdownState>();
            assert_eq!(v.value, 8);
            assert_eq!(v.state, PlayingState::Pending);
        }

        app.emit(Event::Start);
        rt.advance(Duration::from_secs(4));

        {
            let v = app.model::<CountdownState>();
            assert_eq!(v.value, 4);
            assert_eq!(v.state, PlayingState::Playing);
        }

        rt.advance(Duration::from_secs(5));

        {
            let v = app.model::<CountdownState>();
            assert_eq!(v.value, 0);
            assert_eq!(v.state, PlayingState::Pending);
        }
    }
}
