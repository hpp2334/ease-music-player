use std::time::Duration;

use ease_client_shared::{
    backends::{
        music::{ArgUpdateMusicDuration, MusicId},
        music_duration::MusicDuration,
        playlist::PlaylistId,
    },
    uis::preference::PlayMode,
};
use misty_vm::{AppBuilderContext, AsyncTasks, IToHost, Model, ViewModel, ViewModelContext};

use super::{
    common::MusicCommonVM,
    state::CurrentMusicState,
};
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
    Play,
    Pause,
    Stop,
    Total { duration_ms: u64 },
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

    pub(crate) fn request_play(&self, cx: &ViewModelContext, id: MusicId) -> EaseResult<()> {
        let current_playlist = PlaylistCommonVM::of(cx).get_current(cx)?;

        if let Some(current_playlist) = current_playlist {
            self.request_play_impl(cx, id, current_playlist.id())?;
        } else {
            tracing::warn!("current playlist empty");
        }
        Ok(())
    }

    fn request_replay(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let id = cx.model_get(&self.current).id;
        if let Some(current_id) = id {
            self.request_stop(cx)?;
            self.request_play(cx, current_id)?;
        }
        Ok(())
    }

    pub(crate) fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.sync_current_duration(cx)?;
        Ok(())
    }

    fn sync_current_duration(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let mut current = cx.model_mut(&self.current);
        current.current_duration =
            Duration::from_secs(MusicPlayerService::of(cx).get_current_duration_s());
        Ok(())
    }

    fn request_play_next(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request_play_adjacent::<true>(cx)
    }

    fn request_play_previous(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request_play_adjacent::<false>(cx)
    }

    fn request_play_adjacent<const IS_NEXT: bool>(&self, cx: &ViewModelContext) -> EaseResult<()> {
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
                    this.request_play_impl(&cx, adjacent_music.id(), playlist_id)?;
                }
            }
            Ok(())
        });
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
        self.sync_current_duration(cx)?;
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

    fn request_play_impl(
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
                state.lyric_line_index = -1;
            }
            {
                let url = Connector::of(&cx).serve_music_url(&cx, music_id);
                MusicPlayerService::of(&cx).set_music_url(url);
            }
            this.sync_current_duration(&cx)?;
            this.request_resume(&cx)?;

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
                } else if !state.playlist_musics.is_empty()
                    && !state.playlist_musics.iter().any(|v| v.id() == id)
                {
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
        let mut state = cx.model_mut(&self.current);
        state.playing = value;

        if value {
            MusicCommonVM::of(cx).schedule_tick::<true>(cx)?;
        }
        Ok(())
    }

    fn on_player_event(&self, cx: &ViewModelContext, event: &PlayerEvent) -> EaseResult<()> {
        match event {
            PlayerEvent::Complete => self.on_complete(cx)?,
            PlayerEvent::Loading => {}
            PlayerEvent::Loaded => {}
            PlayerEvent::Play => self.update_playing(cx, true)?,
            PlayerEvent::Pause => self.update_playing(cx, false)?,
            PlayerEvent::Stop => self.stop_impl(cx)?,
            PlayerEvent::Total { duration_ms } => {
                self.on_sync_total_duration(cx, Duration::from_millis(*duration_ms))?
            }
        };
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

    fn on_sync_total_duration(&self, cx: &ViewModelContext, duration: Duration) -> EaseResult<()> {
        let (id, playlist_id) = {
            let state = cx.model_get(&self.current);
            match (state.id, state.playlist_id) {
                (Some(id), Some(playlist_id)) => (id, playlist_id),
                _ => return Ok(()),
            }
        };
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            Connector::of(&cx)
                .update_music_total_duration(
                    &cx,
                    playlist_id,
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
