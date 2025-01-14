use std::time::Duration;

use ease_client_shared::backends::{
    connector::ConnectorAction,
    generated::RemoveMusicFromPlaylistMsg,
    music::{Music, MusicId},
    player::PlayerCurrentPlaying,
    playlist::{ArgRemoveMusicFromPlaylist, PlaylistId},
};
use misty_vm::{AppBuilderContext, AsyncTaskPod, AsyncTasks, Model, ViewModel, ViewModelContext};

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    view_models::{connector::Connector, main::MainBodyVM},
    MainAction, ViewAction,
};

use super::{
    control::MusicControlVM,
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
    tick_task: AsyncTaskPod,
}

impl MusicCommonVM {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            current: cx.model(),
            time_to_pause: cx.model(),
            tasks: Default::default(),
            tick_task: Default::default(),
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
            connector
                .request::<RemoveMusicFromPlaylistMsg>(
                    &cx,
                    ArgRemoveMusicFromPlaylist {
                        music_id: id,
                        playlist_id,
                    },
                )
                .await?;
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn remove_current(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let m = cx.model_get(&self.current);
        if let Some(PlayerCurrentPlaying {
            abstr, playlist_id, ..
        }) = m.music.as_ref()
        {
            self.remove(cx, abstr.id(), *playlist_id)
        } else {
            Ok(())
        }
    }

    pub(crate) fn schedule_tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let this = Self::of(cx);
        cx.spawn_in_pod::<_, _, EaseError>(&self.tasks, &self.tick_task, move |cx| async move {
            cx.sleep(Duration::from_secs(1)).await;
            this.tick_task.cancel(&this.tasks);
            this.tick(&cx)?;
            Ok(())
        });
        Ok(())
    }

    fn tick(&self, cx: &ViewModelContext) -> EaseResult<()> {
        if !MainBodyVM::of(cx).visible(cx) {
            self.tick_task.cancel(&self.tasks);
            return Ok(());
        }

        let is_playing = cx.model_get(&self.current).playing;
        let time_to_pause_enabled = cx.model_get(&self.time_to_pause).enabled;

        if is_playing {
            MusicControlVM::of(cx).tick(cx)?;
        }
        if time_to_pause_enabled {
            TimeToPauseVM::of(cx).tick(cx)?;
        }

        if is_playing || time_to_pause_enabled {
            self.schedule_tick(&cx)?;
        } else {
            self.tick_task.cancel(&self.tasks);
        }
        Ok(())
    }

    fn sync_music(&self, cx: &ViewModelContext, music: &Music) -> EaseResult<()> {
        let mut state = cx.model_mut(&self.current);
        if state.id() == Some(music.id()) {
            state.lyric = music.lyric.clone();
        }
        Ok(())
    }
}

impl ViewModel for MusicCommonVM {
    type Event = Action;
    type Error = EaseError;
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
                _ => {}
            },
            Action::VsLoaded => {
                self.tick(cx)?;
            }
            Action::View(action) => match action {
                ViewAction::Main(action) => match action {
                    MainAction::OnMainWinShown => {
                        self.tick(cx)?;
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
