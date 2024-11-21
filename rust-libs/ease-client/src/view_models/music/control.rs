use ease_client_shared::backends::{
    connector::ConnectorAction,
    music::MusicId,
    player::{ArgPlayMusic, ConnectorPlayerAction, PlayMode},
    playlist::Playlist,
};
use misty_vm::{
    AppBuilderContext, AsyncTaskPod, AsyncTasks, IToHost, Model, ViewModel, ViewModelContext,
};

use super::{
    common::MusicCommonVM, lyric::MusicLyricVM, state::CurrentMusicState,
    time_to_pause::TimeToPauseVM,
};
use crate::{
    actions::{event::ViewAction, Widget},
    view_models::main::router::RouterVM,
    RoutesKey,
};
use crate::{
    actions::{Action, WidgetActionType},
    error::{EaseError, EaseResult},
    view_models::connector::Connector,
};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum MusicControlWidget {
    Play,
    Pause,
    PlayPrevious,
    PlayNext,
    Stop,
    Playmode,
    TimeToPause,
}

#[derive(Debug, uniffi::Enum)]
pub enum MusicControlAction {
    Seek { duration_ms: u64 },
}

pub(crate) struct MusicControlVM {
    current: Model<CurrentMusicState>,
    tasks: AsyncTasks,
}

impl MusicControlVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
            tasks: Default::default(),
        }
    }

    pub(crate) fn prepare(
        &self,
        cx: &ViewModelContext,
        playlist: &Playlist,
        id: MusicId,
    ) -> EaseResult<()> {
        let playlist_id = playlist.id();
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .player_play(&cx, ArgPlayMusic { id, playlist_id })
                .await?;
            Ok(())
        });
        RouterVM::of(cx).navigate(cx, RoutesKey::MusicPlayer);
        Ok(())
    }

    pub(crate) fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request_sync_current_duration(cx);
        Ok(())
    }

    fn request_sync_current_duration(&self, cx: &ViewModelContext) {
        let current = self.current.clone();
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            let duration = Connector::of(&cx).player_current_duration(&cx).await?;

            {
                let mut current = cx.model_mut(&current);
                current.current_duration = duration;
            }
            MusicLyricVM::of(&cx).sync_lyric_index(&cx)?;

            Ok(())
        });
    }

    fn request_play_next(&self, cx: &ViewModelContext) {
        {
            let mut state = cx.model_mut(&self.current);
            if let Some(music) = &mut state.music {
                music.cover = music.next_cover.clone();
            }
        }

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).player_play_next(&cx).await?;
            Ok(())
        });
    }

    fn request_play_previous(&self, cx: &ViewModelContext) {
        {
            let mut state = cx.model_mut(&self.current);
            if let Some(music) = &mut state.music {
                music.cover = music.prev_cover.clone();
            }
        }

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).player_play_previous(&cx).await?;
            Ok(())
        });
    }

    fn request_resume(&self, cx: &ViewModelContext) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).player_resume(&cx).await?;
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn request_pause(&self, cx: &ViewModelContext) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).player_pause(&cx).await?;
            Ok(())
        });
        Ok(())
    }

    fn request_stop(&self, cx: &ViewModelContext) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).player_stop(&cx).await?;
            Ok(())
        });
        Ok(())
    }

    fn request_seek(&self, cx: &ViewModelContext, arg: u64) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).player_seek(&cx, arg).await?;
            Ok(())
        });
        Ok(())
    }

    fn update_playmode_to_next(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.current);
        let play_mode = match state.play_mode {
            PlayMode::Single => PlayMode::SingleLoop,
            PlayMode::SingleLoop => PlayMode::List,
            PlayMode::List => PlayMode::ListLoop,
            PlayMode::ListLoop => PlayMode::Single,
        };
        state.play_mode = play_mode;

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).update_playmode(&cx, play_mode).await?;
            Ok(())
        });
        Ok(())
    }

    fn on_player_event(
        &self,
        cx: &ViewModelContext,
        event: &ConnectorPlayerAction,
    ) -> EaseResult<()> {
        match event {
            ConnectorPlayerAction::Current { value } => {
                let current = self.current.clone();
                let id = value.as_ref().map(|v| v.abstr.id());
                {
                    let mut state = cx.model_mut(&current);
                    state.music = value.clone();
                }

                if let Some(id) = id {
                    cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
                        let music = Connector::of(&cx).get_music(&cx, id).await?;

                        if let Some(music) = music {
                            if music.id() == id {
                                {
                                    let mut state: std::cell::RefMut<'_, CurrentMusicState> =
                                        cx.model_mut(&current);
                                    state.lyric = music.lyric;
                                    state.lyric_line_index = -1;
                                }
                            }
                        }

                        Ok(())
                    });
                }
            }
            ConnectorPlayerAction::Playmode { value } => {
                let mut state = cx.model_mut(&self.current);
                state.play_mode = *value;
            }
            ConnectorPlayerAction::Seeked => {}
            ConnectorPlayerAction::Playing { value } => {
                let mut state = cx.model_mut(&self.current);
                state.playing = *value;
            }
        };
        self.request_sync_current_duration(&cx);
        MusicCommonVM::of(&cx).schedule_tick(&cx)?;
        Ok(())
    }
}

impl ViewModel for MusicControlVM {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::MusicControl(action) => match action {
                    MusicControlAction::Seek { duration_ms } => {
                        self.request_seek(cx, *duration_ms)?
                    }
                },
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::MusicControl(widget), WidgetActionType::Click) => match widget {
                        MusicControlWidget::Pause => {
                            self.request_pause(cx)?;
                        }
                        MusicControlWidget::Play => {
                            self.request_resume(cx)?;
                        }
                        MusicControlWidget::PlayNext => {
                            self.request_play_next(cx);
                        }
                        MusicControlWidget::PlayPrevious => {
                            self.request_play_previous(cx);
                        }
                        MusicControlWidget::Stop => {
                            self.request_stop(cx)?;
                        }
                        MusicControlWidget::Playmode => {
                            self.update_playmode_to_next(cx)?;
                        }
                        MusicControlWidget::TimeToPause => {
                            TimeToPauseVM::of(cx).open(cx);
                        }
                    },
                    _ => {}
                },
                _ => {}
            },
            Action::Connector(action) => match action {
                ConnectorAction::Player(action) => self.on_player_event(cx, action)?,
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
