use std::time::Duration;

use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    view_models::connector::Connector,
};
use ease_client_shared::backends::storage::{
    ArgUpsertStorage, StorageConnectionTestResult, StorageId, StorageType,
};
use misty_vm::{AppBuilderContext, AsyncTasks, Model, ViewModel, ViewModelContext};

use super::state::{AllStorageState, EditStorageState};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum StorageUpsertWidget {
    Type { value: StorageType },
    IsAnonymous,
    Alias,
    Address,
    Username,
    Password,
    Remove,
    Test,
    Finish,
}

pub(crate) struct StorageUpsertVM {
    edit: Model<EditStorageState>,
    store: Model<AllStorageState>,
    tasks: AsyncTasks,
}

impl StorageUpsertVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            edit: cx.model(),
            store: cx.model(),
            tasks: Default::default()
        }
    }

    pub(crate) fn prepare_create(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.tasks.cancel_all();
        let mut edit = cx.model_mut(&self.edit);
        edit.info = ArgUpsertStorage {
            id: None,
            addr: Default::default(),
            alias: Default::default(),
            username: Default::default(),
            password: Default::default(),
            is_anonymous: true,
            typ: StorageType::Webdav,
        };
        edit.is_create = true;
        edit.test = StorageConnectionTestResult::None;
        Ok(())
    }

    pub(crate) fn prepare_edit(
        &self,
        cx: &ViewModelContext,
        storage_id: StorageId,
    ) -> EaseResult<()> {
        self.tasks.cancel_all();
        let storage = {
            let model_get = cx.model_get(&self.store);
            let storage = model_get.storages.get(&storage_id);
            match storage {
                Some(storage) => storage.clone(),
                None => return Ok(()),
            }
        };

        let mut edit = cx.model_mut(&self.edit);
        edit.info = ArgUpsertStorage {
            id: Some(storage_id),
            addr: storage.addr.clone(),
            alias: storage.alias.clone(),
            username: storage.username.clone(),
            password: storage.password.clone(),
            is_anonymous: storage.is_anonymous,
            typ: storage.typ.clone(),
        };
        edit.test = StorageConnectionTestResult::None;
        edit.music_count = storage.music_count;
        edit.playlist_count = storage.playlist_count;
        edit.is_create = false;
        Ok(())
    }

    fn remove(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let id = cx.model_get(&self.edit).info.id.unwrap();

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).remove_storage(&cx, id).await?;
            Ok(())
        });
        Ok(())
    }

    fn test(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let arg = cx.model_get(&self.edit).info.clone();
        let edit_model = self.edit.clone();
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            let res = Connector::of(&cx).test_storage(&cx, arg).await?;

            {
                let mut edit = cx.model_mut(&edit_model);
                edit.test = res;
            }

            cx.sleep(Duration::from_secs(3)).await;

            {
                let mut edit = cx.model_mut(&edit_model);
                edit.test = StorageConnectionTestResult::None;
            }

            Ok(())
        });
        Ok(())
    }

    fn finish(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let arg = cx.model_get(&self.edit).info.clone();
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).upsert_storage(&cx, arg).await?;
            Ok(())
        });
        Ok(())
    }
}

   impl ViewModel for StorageUpsertVM {
     type Event = Action;
     type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> Result<(), EaseError> {
        match event {
            Action::View(action) => {
                match action {
                    ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                        (Widget::StorageUpsert(action), WidgetActionType::Click) => match action {
                            StorageUpsertWidget::Type { value } => {
                                let mut form = cx.model_mut(&self.edit);
                                form.info.typ = *value;
                            }
                            StorageUpsertWidget::IsAnonymous => {
                                let mut form = cx.model_mut(&self.edit);
                                let value = &mut form.info.is_anonymous;
                                *value = !*value;
                            }
                            StorageUpsertWidget::Remove => {
                                self.remove(cx)?;
                            }
                            StorageUpsertWidget::Test => {
                                self.test(cx)?;
                            }
                            StorageUpsertWidget::Finish => {
                                self.finish(cx)?;
                            }
                            _ => {
                                unimplemented!()
                            }
                        },
                        (Widget::StorageUpsert(action), WidgetActionType::ChangeText { text }) => {
                            match action {
                                StorageUpsertWidget::Alias => {
                                    let mut form = cx.model_mut(&self.edit);
                                    form.info.alias = text.to_string();
                                }
                                StorageUpsertWidget::Address => {
                                    let mut form = cx.model_mut(&self.edit);
                                    form.info.addr = text.to_string();
                                }
                                StorageUpsertWidget::Username => {
                                    let mut form = cx.model_mut(&self.edit);
                                    form.info.username = text.to_string();
                                }
                                StorageUpsertWidget::Password => {
                                    let mut form = cx.model_mut(&self.edit);
                                    form.info.password = text.to_string();
                                }
                                _ => unimplemented!(),
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            },
            _ => {}
        }
        Ok(())
    }
}
