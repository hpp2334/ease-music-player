use std::{sync::Arc, time::Duration};

use ease_client_shared::backends::{
    connector::{ConnectorAction, IConnectorNotifier},
    generated::*,
    message::{decode_message_payload, encode_message_payload, IMessage, MessagePayload},
    music::*,
    player::{ArgPlayMusic, PlayMode, PlayerCurrentPlaying},
    playlist::*,
    storage::*,
};
use misty_vm::{
    AppBuilderContext, AsyncTasks, AsyncViewModelContext, IToHost, Model, ViewModel,
    ViewModelContext,
};
use state::ConnectorState;

pub mod state;

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    to_host::connector::ConnectorHostService,
};

use super::playlist::state::CurrentPlaylistState;

pub struct Connector {
    state: Model<ConnectorState>,
    current_playlist: Model<CurrentPlaylistState>,
    tasks: AsyncTasks,
}

impl Connector {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            state: cx.model(),
            current_playlist: cx.model(),
            tasks: Default::default(),
        }
    }

    pub fn serve_asset_url(&self, cx: &ViewModelContext, loc: StorageEntryLoc) -> String {
        let state = cx.model_get(&self.state);
        state.serve_asset_url(loc)
    }

    pub fn serve_music_url(&self, cx: &ViewModelContext, id: MusicId) -> String {
        let state = cx.model_get(&self.state);
        state.serve_music_url(id)
    }

    pub fn storage_path(&self, cx: &ViewModelContext) -> String {
        ConnectorHostService::of(cx).storage_path()
    }

    fn init(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let host = ConnectorHostService::of(cx);
        let handle = host.connect(Arc::new(ConnectorNotifier { cx: cx.weak() }));
        {
            let mut state = cx.model_mut(&self.state);
            state.port = host.port();
            state.connector_handle = handle;
        }

        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            this.request::<OnConnectMsg>(&cx, ()).await?;
            this.sync_storages(&cx).await?;
            this.sync_playlist_abstracts(&cx).await?;

            cx.enqueue_emit(Action::VsLoaded);
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn destroy(&self, cx: &ViewModelContext) {
        let handle = cx.model_get(&self.state).connector_handle;
        ConnectorHostService::of(cx).disconnect(handle);
    }

    pub async fn get_music(&self, cx: &ViewModelContext, id: MusicId) -> EaseResult<Option<Music>> {
        self.request::<GetMusicMsg>(cx, id).await
    }

    pub async fn get_playlist(
        &self,
        cx: &ViewModelContext,
        id: PlaylistId,
    ) -> EaseResult<Option<Playlist>> {
        self.request::<GetPlaylistMsg>(cx, id).await
    }

    pub async fn remove_music(
        &self,
        cx: &ViewModelContext,
        id: MusicId,
        playlist_id: PlaylistId,
    ) -> EaseResult<()> {
        self.request::<RemoveMusicFromPlaylistMsg>(
            cx,
            ArgRemoveMusicFromPlaylist {
                playlist_id,
                music_id: id,
            },
        )
        .await?;
        self.sync_playlist(cx, playlist_id).await?;
        self.sync_playlist_abstracts(cx).await?;
        self.sync_storages(cx).await?;
        Ok(())
    }

    pub async fn update_playlist(
        &self,
        cx: &ViewModelContext,
        arg: ArgUpdatePlaylist,
    ) -> EaseResult<()> {
        let id = arg.id;
        self.request::<UpdatePlaylistMsg>(cx, arg).await?;
        self.sync_playlist(cx, id).await?;
        self.sync_playlist_abstracts(cx).await?;
        Ok(())
    }

    pub async fn create_playlist(
        &self,
        cx: &ViewModelContext,
        arg: ArgCreatePlaylist,
    ) -> EaseResult<()> {
        let id = self.request::<CreatePlaylistMsg>(cx, arg).await?;
        self.sync_playlist_abstracts(cx).await?;
        Ok(())
    }

    pub async fn remove_playlist(&self, cx: &ViewModelContext, id: PlaylistId) -> EaseResult<()> {
        self.request::<RemovePlaylistMsg>(cx, id).await?;
        self.sync_playlist_abstracts(cx).await?;
        Ok(())
    }

    pub async fn add_musics_to_playlist(
        &self,
        cx: &ViewModelContext,
        arg: ArgAddMusicsToPlaylist,
    ) -> EaseResult<()> {
        let id = arg.id;
        self.request::<AddMusicsToPlaylistMsg>(cx, arg).await?;
        self.sync_playlist(cx, id).await?;
        self.sync_playlist_abstracts(cx).await?;
        self.sync_storages(cx).await?;
        Ok(())
    }

    pub async fn update_music_lyric(
        &self,
        cx: &ViewModelContext,
        arg: ArgUpdateMusicLyric,
    ) -> EaseResult<()> {
        let id = arg.id;
        self.request::<UpdateMusicLyricMsg>(cx, arg).await?;
        self.sync_music(cx, id).await?;
        Ok(())
    }

    pub async fn list_storage_entry_children(
        &self,
        cx: &ViewModelContext,
        loc: StorageEntryLoc,
    ) -> EaseResult<ListStorageEntryChildrenResp> {
        let resp = self.request::<ListStorageEntryChildrenMsg>(cx, loc).await?;
        Ok(resp)
    }

    pub async fn remove_storage(&self, cx: &ViewModelContext, id: StorageId) -> EaseResult<()> {
        self.request::<RemoveStorageMsg>(cx, id).await?;
        self.sync_storages(cx).await?;
        self.sync_playlist_abstracts(cx).await?;

        if let Some(current_playlist_id) = {
            let state = cx.model_get(&self.current_playlist);
            state.playlist.as_ref().map(|v| v.id())
        } {
            self.sync_playlist(cx, current_playlist_id).await?;
        }
        Ok(())
    }

    pub async fn test_storage(
        &self,
        cx: &ViewModelContext,
        arg: ArgUpsertStorage,
    ) -> EaseResult<StorageConnectionTestResult> {
        self.request::<TestStorageMsg>(cx, arg).await
    }

    pub async fn upsert_storage(
        &self,
        cx: &ViewModelContext,
        arg: ArgUpsertStorage,
    ) -> EaseResult<()> {
        self.request::<UpsertStorageMsg>(cx, arg).await?;
        self.sync_storages(cx).await?;
        Ok(())
    }

    pub async fn update_playmode(&self, cx: &ViewModelContext, arg: PlayMode) -> EaseResult<()> {
        self.request::<UpdatePlaymodeMsg>(cx, arg).await?;
        Ok(())
    }

    pub async fn get_player_current(
        &self,
        cx: &ViewModelContext,
    ) -> EaseResult<Option<PlayerCurrentPlaying>> {
        let current = self.request::<PlayerCurrentMsg>(cx, ()).await?;
        Ok(current)
    }

    pub async fn player_current_duration(&self, cx: &ViewModelContext) -> EaseResult<Duration> {
        let duration = self.request::<PlayerCurrentDurationMsg>(cx, ()).await?;
        Ok(duration)
    }

    pub async fn player_play(&self, cx: &ViewModelContext, arg: ArgPlayMusic) -> EaseResult<()> {
        self.request::<PlayMusicMsg>(cx, arg).await?;
        Ok(())
    }

    pub async fn player_stop(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request::<StopPlayerMsg>(cx, ()).await?;
        Ok(())
    }

    pub async fn player_play_next(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request::<PlayNextMsg>(cx, ()).await?;
        Ok(())
    }

    pub async fn player_play_previous(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request::<PlayPreviousMsg>(cx, ()).await?;
        Ok(())
    }

    pub async fn player_resume(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request::<ResumePlayerMsg>(cx, ()).await?;
        Ok(())
    }

    pub async fn player_pause(&self, cx: &ViewModelContext) -> EaseResult<()> {
        self.request::<PausePlayerMsg>(cx, ()).await?;
        Ok(())
    }

    pub async fn player_seek(&self, cx: &ViewModelContext, ms: u64) -> EaseResult<()> {
        self.request::<PlayerSeekMsg>(cx, ms).await?;
        Ok(())
    }

    async fn sync_music(&self, cx: &ViewModelContext, id: MusicId) -> EaseResult<()> {
        let music = self.get_music(cx, id).await?;
        if let Some(music) = music {
            cx.enqueue_emit(Action::Connector(ConnectorAction::Music(music)));
        }
        Ok(())
    }

    async fn sync_playlist_abstracts(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let playlists = self.request::<GetAllPlaylistAbstractsMsg>(cx, ()).await?;
        cx.enqueue_emit(Action::Connector(ConnectorAction::PlaylistAbstracts(
            playlists,
        )));
        Ok(())
    }

    async fn sync_current_playlist(&self, cx: &ViewModelContext) -> EaseResult<()> {
        if let Some(current_playlist_id) = {
            let state = cx.model_get(&self.current_playlist);
            state.playlist.as_ref().map(|v| v.id())
        } {
            self.sync_playlist(cx, current_playlist_id).await?;
        }
        Ok(())
    }

    async fn sync_playlist(&self, cx: &ViewModelContext, id: PlaylistId) -> EaseResult<()> {
        let playlist = self.request::<GetPlaylistMsg>(cx, id).await?;
        if let Some(playlist) = playlist {
            cx.enqueue_emit(Action::Connector(ConnectorAction::Playlist(playlist)));
        }
        Ok(())
    }

    async fn sync_storages(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let storages = self.request::<ListStorageMsg>(cx, ()).await?;
        cx.enqueue_emit(Action::Connector(ConnectorAction::Storages(storages)));
        Ok(())
    }

    async fn request<S: IMessage>(
        &self,
        cx: &ViewModelContext,
        arg: <S as IMessage>::Argument,
    ) -> EaseResult<<S as IMessage>::Return> {
        let arg = encode_message_payload(arg);
        let ret = ConnectorHostService::of(cx)
            .request(MessagePayload {
                code: S::CODE,
                payload: arg,
            })
            .await?;
        let ret = decode_message_payload(ret.payload);
        Ok(ret)
    }
}

struct ConnectorNotifier {
    cx: AsyncViewModelContext,
}
impl IConnectorNotifier for ConnectorNotifier {
    fn notify(&self, action: ConnectorAction) {
        self.cx.enqueue_emit(Action::Connector(action));
    }
}

impl ViewModel for Connector {
    type Event = Action;
    type Error = EaseError;
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::Init => self.init(cx)?,
            Action::Destroy => self.destroy(cx),
            Action::Connector(action) => match action {
                ConnectorAction::MusicCoverChanged(_)
                | ConnectorAction::MusicTotalDurationChanged(_) => {
                    let this = Self::of(cx);
                    cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
                        this.sync_current_playlist(&cx).await?;
                        this.sync_playlist_abstracts(&cx).await?;
                        Ok(())
                    });
                }
                _ => {}
            },
            _ => {}
        }
        Ok(())
    }
}
