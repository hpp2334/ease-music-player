use std::time::Duration;

use ease_client_shared::backends::{
    music::{Music, MusicId},
    playlist::{Playlist, PlaylistId},
};
use misty_vm::{AppBuilderContext, AsyncTasks, Model, ViewModel, ViewModelContext};

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    view_models::connector::{Connector, ConnectorAction},
};

use super::{
    control::MusicControlVM,
    lyric::MusicLyricVM,
    state::{CurrentMusicState, TimeToPauseState},
    time_to_pause::TimeToPauseVM,
};

#[derive(Debug, uniffi::Enum)]
pub enum MusicCommonAction {
    Tick,
}

pub(crate) struct MusicCommonVM {
    current: Model<CurrentMusicState>,
    time_to_pause: Model<TimeToPauseState>,
    tasks: AsyncTasks,
}

impl MusicCommonVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
            time_to_pause: cx.model(),
            tasks: Default::default(),
        }
    }

    pub(crate) fn remove(
        &self,
        cx: &ViewModelContext,
        id: MusicId,
        playlist_id: PlaylistId,
    ) -> EaseResult<()> {
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            let connector = Connector::of(&cx);
            connector.remove_music(&cx, id, playlist_id).await?;
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn remove_current(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let m = cx.model_get(&self.current);
        if let (Some(current_id), Some(playlist_id)) = (m.id, m.playlist_id) {
            self.remove(cx, current_id, playlist_id)
        } else {
            Ok(())
        }
    }

    pub(crate) fn schedule_tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            cx.sleep(Duration::from_secs(1)).await;
            this.tick(&cx)?;
            Ok(())
        });
        Ok(())
    }

    fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let is_playing = cx.model_get(&self.current).playing;
        let time_to_pause_enabled = cx.model_get(&self.time_to_pause).enabled;

        if is_playing {
            MusicLyricVM::of(cx).tick_lyric_index(cx)?;
            MusicControlVM::of(cx).tick(cx)?;
        }
        if time_to_pause_enabled {
            TimeToPauseVM::of(cx).tick(cx)?;
        }

        if is_playing || time_to_pause_enabled {
            self.schedule_tick(&cx)?;
        }
        Ok(())
    }

    fn sync_music(&self, cx: &ViewModelContext, music: &Music) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.current);
        if state.id == Some(music.id()) {
            let index_music = state.index_musics;

            state.lyric = music.lyric.clone();

            let r = &mut state.playlist_musics[index_music];
            assert!(r.id() == music.id());
            *r = music.music_abstract();
        }
        Ok(())
    }

    fn sync_playlist(&self, cx: &ViewModelContext, playlist: &Playlist) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.current);
        if state.playlist_id == Some(playlist.id()) {
            let id = state.id.unwrap();
            let pos = playlist.musics.iter().position(|v| v.id() == id);

            state.playlist_musics = playlist.musics.clone();
            state.index_musics = pos.unwrap_or(0);
        }

        Ok(())
    }
}

impl ViewModel<Action, EaseError> for MusicCommonVM {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::MusicCommon(action) => match action {
                MusicCommonAction::Tick => {
                    self.tick(cx)?;
                }
            },
            Action::Connector(action) => match action {
                ConnectorAction::Music(music) => {
                    self.sync_music(cx, music)?;
                }
                ConnectorAction::Playlist(playlist) => {
                    self.sync_playlist(cx, playlist)?;
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
