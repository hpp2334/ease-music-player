use std::{rc::Rc, time::Duration};

use ease_client_shared::{
    backends::{
        music::{ArgUpdateMusicCover, ArgUpdateMusicDuration, MusicId},
        music_duration::MusicDuration,
        playlist::{Playlist, PlaylistId},
    },
    uis::preference::PlayMode,
};
use misty_vm::{AppBuilderContext, AsyncTasks, IToHost, Model, ViewModel, ViewModelContext};

use super::{
    common::MusicCommonVM,
    state::{CurrentMusicState, QueueMusic},
};
use crate::{
    actions::{event::ViewAction, Widget},
    to_host::player::{MusicPlayerService, MusicToPlay},
    view_models::{
        connector::ConnectorAction, main::router::RouterVM, playlist::common::PlaylistCommonVM,
    },
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
}

#[derive(Debug, uniffi::Enum)]
pub enum MusicControlAction {
    Seek { duration_ms: u64 },
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum PlayerEvent {
    Complete,
    Loading,
    Loaded,
    Play,
    Pause,
    Stop,
    Seek,
    Total { id: MusicId, duration_ms: u64 },
    Cover { id: MusicId, buffer: Vec<u8> },
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
        let current_index = playlist
            .musics
            .iter()
            .position(|m| m.id() == id)
            .unwrap_or(0);
        let to_play = QueueMusic {
            id,
            playlist_id: playlist.id(),
            queue: Rc::new(playlist.musics.clone()),
            index: current_index,
        };

        self.request_play_impl(cx, to_play)?;
        RouterVM::of(cx).navigate(cx, RoutesKey::MusicPlayer);
        Ok(())
    }

    fn request_replay(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let has_music = cx.model_get(&self.current).music.is_some();
        if has_music {
            self.request_pause(cx)?;
            self.request_seek(cx, 0)?;
            self.request_resume(cx)?;
        }
        Ok(())
    }

    pub(crate) fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.sync_current_duration(cx);
        Ok(())
    }

    fn sync_current_duration(&self, cx: &ViewModelContext) {
        let mut current = cx.model_mut(&self.current);
        current.current_duration =
            Duration::from_secs(MusicPlayerService::of(cx).get_current_duration_s());
    }

    fn request_play_next(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request_play_adjacent::<true>(cx)
    }

    fn request_play_previous(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request_play_adjacent::<false>(cx)
    }

    fn request_play_adjacent<const IS_NEXT: bool>(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let (music, can_play) = {
            let state = cx.model_get(&self.current);
            (
                state.music.clone(),
                if IS_NEXT {
                    state.can_play_next()
                } else {
                    state.can_play_previous()
                },
            )
        };

        if music.is_none() {
            return Ok(());
        }
        let QueueMusic {
            id: current_music_id,
            playlist_id,
            queue,
            ..
        } = music.unwrap();

        if !can_play {
            return Ok(());
        }

        let current_index = queue
            .iter()
            .position(|m| m.id() == current_music_id)
            .unwrap_or(0);
        let adjacent_index = if IS_NEXT {
            if current_index + 1 >= queue.len() {
                0
            } else {
                current_index + 1
            }
        } else {
            if current_index == 0 {
                queue.len() - 1
            } else {
                current_index - 1
            }
        };
        if let Some(adjacent_music) = queue.get(adjacent_index) {
            let to_play = QueueMusic {
                id: adjacent_music.id(),
                playlist_id,
                queue,
                index: adjacent_index,
            };

            self.request_play_impl(&cx, to_play)?;
        }
        Ok(())
    }

    fn request_resume(&self, cx: &ViewModelContext) -> EaseResult<()> {
        MusicPlayerService::of(cx).resume();
        MusicCommonVM::of(cx).schedule_tick::<true>(cx)?;
        Ok(())
    }

    pub(crate) fn request_pause(&self, cx: &ViewModelContext) -> EaseResult<()> {
        MusicPlayerService::of(cx).pause();
        Ok(())
    }

    fn request_stop(&self, cx: &ViewModelContext) -> EaseResult<()> {
        MusicPlayerService::of(cx).stop();
        Ok(())
    }

    fn stop_impl(&self, cx: &ViewModelContext) -> EaseResult<()> {
        {
            let mut state = cx.model_mut(&self.current);
            let playmode = state.play_mode;
            *state = Default::default();
            state.play_mode = playmode;
        }
        self.update_playing(cx, false)?;
        Ok(())
    }

    fn request_seek(&self, cx: &ViewModelContext, arg: u64) -> EaseResult<()> {
        MusicPlayerService::of(cx).seek(arg);
        self.sync_current_duration(cx);
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
            Connector::of(&cx)
                .update_preference_playmode(&cx, play_mode)
                .await?;
            Ok(())
        });
        Ok(())
    }

    fn request_play_impl(&self, cx: &ViewModelContext, to_play: QueueMusic) -> EaseResult<()> {
        let this = Self::of(cx);

        let prev_music = cx.model_get(&this.current).music.clone();

        if prev_music.is_some()
            && prev_music.as_ref().map(|v| v.id).unwrap() == to_play.id
            && prev_music.as_ref().map(|v| v.playlist_id).unwrap() == to_play.playlist_id
        {
            return Ok(());
        }

        let music = to_play.queue[to_play.index].clone();

        {
            let mut state = cx.model_mut(&this.current);
            state.music = Some(to_play.clone());
            state.lyric = None;
            state.lyric_line_index = -1;
        }
        {
            let url = Connector::of(&cx).serve_music_url(&cx, to_play.id);
            let item = MusicToPlay {
                id: to_play.id,
                title: music.title().to_string(),
                url,
                cover_url: music.cover_url,
            };
            MusicPlayerService::of(&cx).set_music_url(item);
        }
        this.sync_current_duration(&cx);
        this.request_resume(&cx)?;

        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            let music = Connector::of(&cx).get_music(&cx, to_play.id).await?;
            if music.is_none() {
                return Ok(());
            }
            let music = music.unwrap();

            {
                let mut state = cx.model_mut(&this.current);
                if state.id() == Some(music.id()) {
                    state.lyric = music.lyric.clone();
                    state.lyric_line_index = -1;
                }
            }
            Ok(())
        });
        Ok(())
    }

    fn stop_if_invalid(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let is_valid = {
            let state = cx.model_get(&self.current);
            if let Some(to_play) = &state.music {
                if !PlaylistCommonVM::of(cx).has_playlist(cx, to_play.playlist_id) {
                    false
                } else if !to_play.queue.iter().any(|v| v.id() == to_play.id) {
                    false
                } else {
                    true
                }
            } else {
                true
            }
        };
        if !is_valid {
            self.request_stop(cx)?;
        }
        Ok(())
    }

    fn update_playing(&self, cx: &ViewModelContext, value: bool) -> EaseResult<()> {
        {
            let mut state = cx.model_mut(&self.current);
            state.playing = value;
        }

        if value {
            MusicCommonVM::of(cx).schedule_tick::<true>(cx)?;
        }
        Ok(())
    }

    fn on_player_event(&self, cx: &ViewModelContext, event: &PlayerEvent) -> EaseResult<()> {
        match event {
            PlayerEvent::Complete => self.on_complete(cx)?,
            PlayerEvent::Loading | PlayerEvent::Loaded | PlayerEvent::Seek => {}
            PlayerEvent::Play => self.update_playing(cx, true)?,
            PlayerEvent::Pause => self.update_playing(cx, false)?,
            PlayerEvent::Stop => self.stop_impl(cx)?,
            PlayerEvent::Total { id, duration_ms } => {
                self.on_sync_total_duration(cx, *id, Duration::from_millis(*duration_ms))?
            }
            PlayerEvent::Cover { id, buffer } => {
                self.on_sync_cover(cx, *id, buffer.clone())?;
            }
        };
        self.sync_current_duration(&cx);
        MusicCommonVM::of(cx).schedule_tick::<true>(cx)?;
        Ok(())
    }

    fn on_complete(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let play_mode = cx.model_get(&self.current).play_mode;
        match play_mode {
            PlayMode::Single => {
                self.update_playing(cx, false)?;
                self.request_pause(cx)?;
                self.request_seek(cx, 0)?
            }
            PlayMode::SingleLoop => self.request_replay(cx)?,
            PlayMode::List | PlayMode::ListLoop => self.request_play_next(cx)?,
        }
        Ok(())
    }

    fn on_sync_cover(&self, cx: &ViewModelContext, id: MusicId, cover: Vec<u8>) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .update_music_cover(&cx, ArgUpdateMusicCover { id, cover })
                .await?;
            Ok(())
        });
        Ok(())
    }

    fn on_sync_total_duration(
        &self,
        cx: &ViewModelContext,
        id: MusicId,
        duration: Duration,
    ) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .update_music_total_duration(
                    &cx,
                    ArgUpdateMusicDuration {
                        id,
                        duration: MusicDuration::new(duration),
                    },
                )
                .await?;
            Ok(())
        });
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
                            self.request_play_next(cx)?;
                        }
                        MusicControlWidget::PlayPrevious => {
                            self.request_play_previous(cx)?;
                        }
                        MusicControlWidget::Stop => {
                            self.request_stop(cx)?;
                        }
                        MusicControlWidget::Playmode => {
                            self.update_playmode_to_next(cx)?;
                        }
                    },
                    _ => {}
                },
                ViewAction::Player(event) => self.on_player_event(cx, event)?,
                _ => {}
            },
            Action::Connector(action) => match action {
                ConnectorAction::Playlist(_) | ConnectorAction::PlaylistAbstracts(_) => {
                    self.stop_if_invalid(cx)?;
                }
                ConnectorAction::Preference(preference) => {
                    let mut current = cx.model_mut(&self.current);
                    current.play_mode = preference.playmode;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
