use std::convert::Infallible;

use misty_vm::{App, AppBuilderContext, IAsyncRuntimeAdapter, Model, ViewModel, ViewModelContext};

#[derive(Debug, Clone)]
enum TodoEvent {
    AddButtonClicked,
    RemoveButtonClicked { index: usize },
    ItemTextChanged { index: usize, text: String },
    MarkCompleteClicked { index: usize },
}

#[derive(Debug, Clone, Default)]
struct TodoItem {
    text: String,
    completed: bool,
}

#[derive(Debug, Clone, Default)]
struct TodoListState {
    items: Vec<TodoItem>,
}

struct TodoListVM {
    state: Model<TodoListState>,
}

impl TodoListVM {
    fn new(cx: &mut AppBuilderContext) -> Self {
        Self { state: cx.model() }
    }
}

impl ViewModel<TodoEvent, Infallible> for TodoListVM {
    fn on_event(&self, cx: &ViewModelContext, event: &TodoEvent) -> Result<(), Infallible> {
        match event {
            TodoEvent::AddButtonClicked => {
                let mut state = cx.model_mut(&self.state);
                state.items.push(TodoItem::default());
            }
            TodoEvent::RemoveButtonClicked { index } => {
                let mut state = cx.model_mut(&self.state);
                if *index < state.items.len() {
                    state.items.remove(*index);
                }
            }
            TodoEvent::ItemTextChanged { index, text } => {
                let mut state = cx.model_mut(&self.state);
                if let Some(item) = state.items.get_mut(*index) {
                    item.text = text.clone();
                }
            }
            TodoEvent::MarkCompleteClicked { index } => {
                let mut state = cx.model_mut(&self.state);
                if let Some(item) = state.items.get_mut(*index) {
                    item.completed = !item.completed;
                }
            }
        }
        Ok(())
    }
}

fn build_app(adapter: impl IAsyncRuntimeAdapter) -> App {
    let app = App::builder()
        .with_view_models(|cx, builder| {
            builder.add(TodoListVM::new(cx));
        })
        .with_async_runtime_adapter(adapter)
        .build();
    app
}

#[cfg(test)]
mod tests {
    use misty_vm_test::AsyncRuntime;

    use super::*;

    #[test]
    fn test_todo_list() {
        let rt = AsyncRuntime::new();
        let _guard = rt.enter();

        let app = build_app(rt.clone());

        // Add a new item
        app.emit(TodoEvent::AddButtonClicked);

        // Change the text of the first item
        app.emit(TodoEvent::ItemTextChanged {
            index: 0,
            text: "Buy groceries".to_string(),
        });

        // Mark the first item as complete
        app.emit(TodoEvent::MarkCompleteClicked { index: 0 });

        // Add another item
        app.emit(TodoEvent::AddButtonClicked);
        app.emit(TodoEvent::ItemTextChanged {
            index: 1,
            text: "Do laundry".to_string(),
        });

        // Check the state
        {
            let state = app.model::<TodoListState>();
            assert_eq!(state.items.len(), 2);
            assert_eq!(state.items[0].text, "Buy groceries");
            assert_eq!(state.items[0].completed, true);
            assert_eq!(state.items[1].text, "Do laundry");
            assert_eq!(state.items[1].completed, false);
        }

        // Remove the first item
        app.emit(TodoEvent::RemoveButtonClicked { index: 0 });

        // Check the state again
        {
            let state = app.model::<TodoListState>();
            assert_eq!(state.items.len(), 1);
            assert_eq!(state.items[0].text, "Do laundry");
            assert_eq!(state.items[0].completed, false);
        }
    }

    #[test]
    fn test_mark_complete_toggle() {
        let rt = AsyncRuntime::new();
        let _guard = rt.enter();

        let app = build_app(rt.clone());

        // Add a new item
        app.emit(TodoEvent::AddButtonClicked);
        app.emit(TodoEvent::ItemTextChanged {
            index: 0,
            text: "Test item".to_string(),
        });

        // Mark as complete
        app.emit(TodoEvent::MarkCompleteClicked { index: 0 });

        {
            let state = app.model::<TodoListState>();
            assert_eq!(state.items[0].completed, true);
        }

        // Toggle back to incomplete
        app.emit(TodoEvent::MarkCompleteClicked { index: 0 });

        {
            let state = app.model::<TodoListState>();
            assert_eq!(state.items[0].completed, false);
        }
    }

    #[test]
    fn test_multiple_items() {
        let rt = AsyncRuntime::new();
        let _guard = rt.enter();

        let app = build_app(rt.clone());

        // Add multiple items
        for i in 0..5 {
            app.emit(TodoEvent::AddButtonClicked);
            app.emit(TodoEvent::ItemTextChanged {
                index: i,
                text: format!("Item {}", i + 1),
            });
        }

        // Check all items are added
        {
            let state = app.model::<TodoListState>();
            assert_eq!(state.items.len(), 5);
            for (i, item) in state.items.iter().enumerate() {
                assert_eq!(item.text, format!("Item {}", i + 1));
                assert_eq!(item.completed, false);
            }
        }

        // Remove middle item
        app.emit(TodoEvent::RemoveButtonClicked { index: 2 });

        // Check item was removed
        {
            let state = app.model::<TodoListState>();
            assert_eq!(state.items.len(), 4);
            assert_eq!(state.items[2].text, "Item 4");
        }
    }
}
