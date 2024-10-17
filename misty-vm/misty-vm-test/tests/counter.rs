use std::convert::Infallible;

use misty_vm::{App, AppBuilderContext, Model, ViewModel, ViewModelContext};

enum Event {
    Increase,
    Decrease,
}

#[derive(Default)]
struct Counter {
    pub counter: u32,
}

struct CounterVM {
    counter: Model<Counter>,
}

impl CounterVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            counter: cx.model(),
        }
    }
}

impl ViewModel<Event, Infallible> for CounterVM {
    fn on_event(&self, cx: &ViewModelContext, e: &Event) -> Result<(), Infallible> {
        match e {
            Event::Increase => {
                let mut value = cx.model_mut(&self.counter);
                value.counter += 1;
            }
            Event::Decrease => {
                let mut value = cx.model_mut(&self.counter);
                value.counter -= 1;
            }
        }

        Ok(())
    }
}

fn build_app() -> App {
    App::builder()
        .with_view_models(|cx, builder| {
            builder.add(CounterVM::new(cx));
        })
        .build()
}

#[cfg(test)]
mod tests {
    use crate::{build_app, Counter, Event};

    #[test]
    fn incr_1() {
        let app = build_app();
        app.emit(Event::Increase);

        {
            let v = app.model::<Counter>();
            assert_eq!(v.counter, 1);
        }
    }

    #[test]
    fn incr_2() {
        let app = build_app();
        app.emit(Event::Increase);
        app.emit(Event::Increase);

        {
            let v = app.model::<Counter>();
            assert_eq!(v.counter, 2);
        }
    }

    #[test]
    fn incr_decr() {
        let app = build_app();
        app.emit(Event::Increase);
        app.emit(Event::Decrease);

        {
            let v = app.model::<Counter>();
            assert_eq!(v.counter, 0);
        }
    }
}
