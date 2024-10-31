use crate::{
    actions::{event::ViewAction, Action, Widget, WidgetActionType},
    error::{EaseError, EaseResult},
    view_models::{connector::Connector, storage::import::StorageImportVM},
};
use ease_client_shared::{
    backends::{music::ArgUpdateMusicLyric, storage::StorageEntryLoc},
    uis::storage::CurrentStorageImportType,
};
use misty_vm::{AppBuilderContext, AsyncTasks, Model, ViewModel, ViewModelContext};

use super::state::CurrentMusicState;

#[derive(Debug, Clone, uniffi::Enum)]
pub enum MusicLyricWidget {
    Add,
    Remove,
}

pub(crate) struct MusicLyricVM {
    current: Model<CurrentMusicState>,
    tasks: AsyncTasks,
}

impl MusicLyricVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
            tasks: Default::default(),
        }
    }

    fn update_loc_impl(
        &self,
        cx: &ViewModelContext,
        loc: Option<StorageEntryLoc>,
    ) -> EaseResult<()> {
        let id = cx.model_get(&self.current).id;
        if id.is_none() {
            return Ok(());
        }
        let id = id.unwrap();

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .update_music_lyric(&cx, ArgUpdateMusicLyric { id, lyric_loc: loc })
                .await?;

            Ok(())
        });
        Ok(())
    }

    fn remove(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.update_loc_impl(cx, None)
    }

    fn prepare_storage(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let id = cx.model_get(&self.current).id;
        if let Some(id) = id {
            StorageImportVM::of(cx)
                .prepare(cx, CurrentStorageImportType::CurrentMusicLyrics { id })?;
        }
        Ok(())
    }

    pub(crate) fn handle_import_lyric(
        &self,
        cx: &ViewModelContext,
        loc: StorageEntryLoc,
    ) -> EaseResult<()> {
        self.update_loc_impl(cx, Some(loc))
    }

    pub(crate) fn tick_lyric_index(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.current);
        if state.lyric.is_none() {
            return Ok(());
        }

        let duration = state.current_duration;
        let lyrics = state.lyric.as_ref().unwrap();

        let i = lyrics
            .data
            .lines
            .binary_search_by(|(line_duration, _)| line_duration.cmp(&duration))
            .map(|i| i as i32)
            .unwrap_or_else(|i| i as i32 - 1);
        state.lyric_line_index = i;
        Ok(())
    }
}

impl ViewModel for MusicLyricVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::MusicLyric(action), WidgetActionType::Click) => match action {
                        MusicLyricWidget::Add => {
                            self.prepare_storage(cx)?;
                        }
                        MusicLyricWidget::Remove => {
                            self.remove(cx)?;
                        }
                    },
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
