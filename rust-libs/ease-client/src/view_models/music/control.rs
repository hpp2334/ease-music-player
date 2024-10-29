use std::time::Duration;

use ease_client_shared::{
    backends::{music::{ArgUpdateMusicDuration, MusicId}, music_duration::MusicDuration, playlist::PlaylistId},
    uis::preference::PlayMode,
};
use misty_vm::{AppBuilderContext, AsyncTasks, IToHost, Model, ViewModel, ViewModelContext};

use super::{common::MusicCommonAction, state::CurrentMusicState};
use crate::{
    actions::{event::ViewAction, Widget},
    to_host::player::MusicPlayerService,
    view_models::{connector::ConnectorAction, playlist::common::PlaylistCommonVM},
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

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum PlayerEvent {
    Complete,
    Loading,
    Loaded,
    Total {
        duration_ms: u64
    }
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

    pub(crate) fn play(&self, cx: &ViewModelContext, id: MusicId) -> EaseResult<()> {
        let current_playlist_id = cx.model_get(&self.current).playlist_id;

        if let Some(playlist_id) = current_playlist_id {
            self.play_impl(cx, id, playlist_id)?;
        }
        Ok(())
    }

    fn replay(&self, cx: &ViewModelContext) -> EaseResult<()> {
        if let Some(current_id) = cx.model_get(&self.current).id {
            self.stop(cx)?;
            self.play(cx, current_id)?;
        }
        Ok(())
    }

    pub(crate) fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut current = cx.model_mut(&self.current);
        current.current_duration = Duration::from_secs(MusicPlayerService::of(cx).get_current_duration_s());
        Ok(())
    }

    fn play_next(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.play_adjacent::<true>(cx)
    }

    fn play_previous(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.play_adjacent::<false>(cx)
    }

    fn play_adjacent<const IS_NEXT: bool>(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let (current_music_id, playlist_id, can_play) = {
            let state = cx.model_get(&self.current);
            (
                state.id,
                state.playlist_id,
                if IS_NEXT {
                    state.can_play_next()
                } else {
                    state.can_play_previous()
                },
            )
        };

        let (current_music_id, playlist_id) = match (current_music_id, playlist_id) {
            (Some(u), Some(v)) => (u, v),
            _ => return Ok(()),
        };
        if !can_play {
            return Ok(());
        }

        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            let playlist = Connector::of(&cx).get_playlist(&cx, playlist_id).await?;
            if let Some(playlist) = playlist {
                let ordered_musics = &playlist.musics;
                let current_index = ordered_musics
                    .iter()
                    .position(|m| m.id() == current_music_id)
                    .unwrap_or(0);
                let adjacent_index = if IS_NEXT {
                    if current_index + 1 >= ordered_musics.len() {
                        0
                    } else {
                        current_index + 1
                    }
                } else {
                    if current_index == 0 {
                        ordered_musics.len() - 1
                    } else {
                        current_index - 1
                    }
                };
                if let Some(adjacent_music) = ordered_musics.get(adjacent_index) {
                    this.play_impl(&cx, adjacent_music.id(), playlist_id)?;
                }
            }
            Ok(())
        });
        Ok(())
    }

    fn resume(&self, cx: &ViewModelContext) -> EaseResult<()> {
        MusicPlayerService::of(cx).resume();
        cx.enqueue_emit::<_, EaseError>(Action::MusicCommon(MusicCommonAction::Tick));
        Ok(())
    }

    fn pause(&self, cx: &ViewModelContext) -> EaseResult<()> {
        MusicPlayerService::of(cx).pause();
        Ok(())
    }

    fn stop(&self, cx: &ViewModelContext) -> EaseResult<()> {
        {
            let mut state = cx.model_mut(&self.current);
            state.id = None;
            state.playlist_id = None;
        }
        MusicPlayerService::of(cx).stop();
        Ok(())
    }

    fn seek(&self, cx: &ViewModelContext, arg: u64) -> EaseResult<()> {
        MusicPlayerService::of(cx).seek(arg);
        Ok(())
    }

    fn update_playmode_to_next(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.current);
        state.play_mode = match state.play_mode {
            PlayMode::Single => PlayMode::SingleLoop,
            PlayMode::SingleLoop => PlayMode::List,
            PlayMode::List => PlayMode::ListLoop,
            PlayMode::ListLoop => PlayMode::Single,
        };
        Ok(())
    }

    fn play_impl(
        &self,
        cx: &ViewModelContext,
        music_id: MusicId,
        playlist_id: PlaylistId,
    ) -> EaseResult<()> {
        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            let music = Connector::of(&cx).get_music(&cx, music_id).await?;
            let playlist = Connector::of(&cx).get_playlist(&cx, playlist_id).await?;
            if music.is_none() || playlist.is_none() {
                return Ok(());
            }
            let music = music.unwrap();
            let playlist = playlist.unwrap();

            let prev_current_music_id = cx.model_get(&this.current).id;
            let prev_playlist_id = cx.model_get(&this.current).playlist_id;

            if prev_current_music_id.is_some()
                && prev_current_music_id.as_ref().unwrap() == &music_id
                && prev_playlist_id == Some(playlist_id)
            {
                return Ok(());
            }

            let index_musics = playlist
                .musics
                .iter()
                .position(|m| m.id() == music.id())
                .unwrap();
            {
                let mut state = cx.model_mut(&this.current);
                state.id = Some(music_id);
                state.playlist_id = Some(playlist_id);
                state.playlist_musics = playlist.musics.clone();
                state.index_musics = index_musics;
                state.lyric = music.lyric;
            }
            this.resume(&cx)?;

            Ok(())
        });
        Ok(())
    }

    fn stop_if_invalid(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let is_valid = {
            let state = cx.model_get(&self.current);
            if let (Some(id), Some(playlist_id)) = (state.id, state.playlist_id) {
                if !PlaylistCommonVM::of(cx).has_playlist(cx, playlist_id) {
                    false
                } else if !state.playlist_musics.iter().any(|v| v.id() == id) {
                    false
                } else {
                    true
                }
            } else {
                true
            }
        };
        if !is_valid {
            self.stop(cx)?;
        }
        Ok(())
    }

    fn on_complete(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let play_mode = cx.model_get(&self.current).play_mode;
        match play_mode {
            PlayMode::Single => self.stop(cx)?,
            PlayMode::SingleLoop => self.replay(cx)?,
            PlayMode::List | PlayMode::ListLoop => self.play_next(cx)?,
        }
        Ok(())
    }

    fn on_sync_total_duration(&self, cx: &ViewModelContext, duration: Duration) -> EaseResult<()> {
        let id = {
            let state = cx.model_get(&self.current);
            match state.id {
                Some(id) => id,
                None => return Ok(())
            }
        };
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx).update_music_total_duration(&cx, ArgUpdateMusicDuration {
                id,
                duration: MusicDuration::new(duration)
            }).await?;
            Ok(())
        });
        Ok(())
    }
}

impl ViewModel<Action, EaseError> for MusicControlVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::View(action) => match action {
                ViewAction::MusicControl(action) => match action {
                    MusicControlAction::Seek { duration_ms } => self.seek(cx, *duration_ms)?,
                },
                ViewAction::Widget(action) => match (&action.widget, &action.typ) {
                    (Widget::MusicControl(widget), WidgetActionType::Click) => match widget {
                        MusicControlWidget::Pause => {
                            self.pause(cx)?;
                        }
                        MusicControlWidget::Play => {
                            self.resume(cx)?;
                        }
                        MusicControlWidget::PlayNext => {
                            self.play_next(cx)?;
                        }
                        MusicControlWidget::PlayPrevious => {
                            self.play_previous(cx)?;
                        }
                        MusicControlWidget::Stop => {
                            self.stop(cx)?;
                        }
                        MusicControlWidget::Playmode => {
                            self.update_playmode_to_next(cx)?;
                        }
                    },
                    _ => {}
                },
                ViewAction::Player(event) => match event {
                    PlayerEvent::Complete => self.on_complete(cx)?,
                    PlayerEvent::Loading => {}
                    PlayerEvent::Loaded => {},
                    PlayerEvent::Total { duration_ms } => self.on_sync_total_duration(cx, Duration::from_millis(*duration_ms))?,
                },
                _ => {}
            },
            Action::Connector(action) => match action {
                ConnectorAction::Playlist(_) | ConnectorAction::PlaylistAbstracts(_) => {
                    self.stop_if_invalid(cx)?;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
