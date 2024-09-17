use std::convert::Infallible;

pub use global_states::{CheckedState, TodolistAllocState, TodolistItem, TodolistState};
use misty_vm::{controllers::MistyControllerContext, states::MistyStateTrait};

mod global_states {
    use std::collections::{HashMap, HashSet};

    use misty_vm::MistyState;

    #[derive(Debug, Clone)]
    pub struct TodolistItem {
        pub id: i32,
        pub title: String,
        pub done: bool,
    }

    #[derive(Debug, Clone, Default, MistyState)]
    pub struct TodolistState {
        pub map: HashMap<i32, TodolistItem>,
    }

    #[derive(Debug, Clone, Default, MistyState)]
    pub struct CheckedState {
        pub set: HashSet<i32>,
    }

    #[derive(Debug, Clone, MistyState)]
    pub struct TodolistAllocState {
        pub alloc: i32,
    }

    impl Default for TodolistAllocState {
        fn default() -> Self {
            Self { alloc: 1 }
        }
    }
}

mod view_model_states {
    #[derive(Debug, Clone)]
    pub struct TodolistItem {
        pub id: i32,
        pub checked: bool,
        pub title: String,
        pub done: bool,
    }

    #[derive(Debug, Clone, Default)]
    pub struct Todolist {
        pub list: Vec<TodolistItem>,
    }

    #[derive(Debug, Clone, Default)]
    pub struct Root {
        pub todolist: Todolist,
    }
}

pub struct ArgAddTodolistItem {
    pub title: String,
}

fn controller_add_todolist_item(
    ctx: MistyControllerContext,
    arg: ArgAddTodolistItem,
) -> Result<(), Infallible> {
    let alloc_id = TodolistAllocState::update(&ctx, |alloc_state| {
        let id = alloc_state.alloc;
        alloc_state.alloc += 1;
        id
    });

    TodolistState::update(&ctx, |state| {
        state.map.insert(
            alloc_id,
            TodolistItem {
                id: alloc_id,
                title: arg.title,
                done: false,
            },
        );
    });
    Ok(())
}

fn controller_check_todolist_item(ctx: MistyControllerContext, id: i32) -> Result<(), Infallible> {
    CheckedState::update(&ctx, move |state| {
        if state.set.contains(&id) {
            state.set.remove(&id);
        } else {
            state.set.insert(id);
        }
    });
    Ok(())
}

fn controller_remove_checked(ctx: MistyControllerContext, _arg: ()) -> Result<(), Infallible> {
    let checked = CheckedState::update(&ctx, |state| {
        let ret = state.set.clone();
        state.set.clear();
        return ret;
    });
    TodolistState::update(&ctx, |state| {
        for id in checked.into_iter() {
            state.map.remove(&id);
        }
    });
    Ok(())
}

fn todolist_view_model(
    (list, checked): (&TodolistState, &CheckedState),
    root: &mut view_model_states::Root,
) {
    let mut vlist: view_model_states::Todolist = Default::default();
    for (id, item) in list.map.clone().into_iter() {
        vlist.list.push(view_model_states::TodolistItem {
            id,
            title: item.title,
            done: false,
            checked: checked.set.contains(&id),
        });
    }

    root.todolist = vlist;
}

#[cfg(test)]
mod test {
    use misty_vm::{
        misty_states, services::MistyServiceManager, states::MistyStateManager,
        views::MistyViewModelManager,
    };
    use misty_vm_test::{TestApp, TestAppContainer};

    use crate::{
        controller_add_todolist_item, controller_check_todolist_item, controller_remove_checked,
        todolist_view_model, view_model_states, ArgAddTodolistItem, CheckedState,
        TodolistAllocState, TodolistState,
    };

    fn build_app() -> TestApp<view_model_states::Root> {
        let app_container = TestAppContainer::new(|changed, state| {
            *state = changed;
        });

        let view_manager = MistyViewModelManager::builder()
            .register(todolist_view_model)
            .build();
        let state_manager = MistyStateManager::new(misty_states!(
            CheckedState,
            TodolistAllocState,
            TodolistState
        ));
        let service_manager = MistyServiceManager::builder().build();

        let app = TestApp::new(view_manager, service_manager, state_manager, app_container);
        app
    }

    #[test]
    fn test_add() {
        let app = build_app();
        app.app().call_controller(
            controller_add_todolist_item,
            ArgAddTodolistItem {
                title: "Math".to_string(),
            },
        );

        let view = app.state();
        assert_eq!(view.todolist.list.len(), 1);
        assert_eq!(view.todolist.list[0].id, 1);
        assert_eq!(view.todolist.list[0].title, "Math");
        assert_eq!(view.todolist.list[0].checked, false);
    }

    #[test]
    fn test_add_check() {
        let app = build_app();
        app.app().call_controller(
            controller_add_todolist_item,
            ArgAddTodolistItem {
                title: "Math".to_string(),
            },
        );
        app.app().call_controller(controller_check_todolist_item, 1);

        let view = app.state();
        assert_eq!(view.todolist.list.len(), 1);
        assert_eq!(view.todolist.list[0].id, 1);
        assert_eq!(view.todolist.list[0].title, "Math");
        assert_eq!(view.todolist.list[0].checked, true);
    }

    #[test]
    fn test_add_check_remove() {
        let app = build_app();
        app.app().call_controller(
            controller_add_todolist_item,
            ArgAddTodolistItem {
                title: "Math".to_string(),
            },
        );
        app.app().call_controller(
            controller_add_todolist_item,
            ArgAddTodolistItem {
                title: "English".to_string(),
            },
        );
        app.app().call_controller(controller_check_todolist_item, 1);
        app.app().call_controller(controller_remove_checked, ());

        let view = app.state();
        assert_eq!(view.todolist.list.len(), 1);
        assert_eq!(view.todolist.list[0].id, 2);
        assert_eq!(view.todolist.list[0].title, "English");
        assert_eq!(view.todolist.list[0].checked, false);
    }
}
