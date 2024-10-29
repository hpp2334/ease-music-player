
use ease_client_shared::backends::{
    app::ArgInitializeApp,
    message::{decode_message_payload, encode_message_payload, IMessage, MessagePayload},
    music::{ArgUpdateMusicDuration, ArgUpdateMusicLyric, GetMusicMsg, Music, MusicId, UpdateMusicDurationMsg, UpdateMusicLyricMsg},
    playlist::{
        AddMusicsToPlaylistMsg, ArgAddMusicsToPlaylist, ArgCreatePlaylist,
        ArgRemoveMusicFromPlaylist, ArgUpdatePlaylist, CreatePlaylistMsg,
        GetAllPlaylistAbstractsMsg, GetPlaylistMsg, Playlist, PlaylistAbstract, PlaylistId,
        RemoveMusicsFromPlaylistMsg, RemovePlaylistMsg, UpdatePlaylistMsg,
    },
    storage::{
        ArgUpsertStorage, ListStorageEntryChildrenMsg, ListStorageEntryChildrenResp,
        ListStorageMsg, RemoveStorageMsg, Storage, StorageConnectionTestResult, StorageEntryLoc,
        StorageId, TestStorageMsg, UpsertStorageMsg,
    },
};
use misty_vm::{AppBuilderContext, AsyncTasks, IToHost, Model, ViewModel, ViewModelContext};
use state::ConnectorState;

pub mod state;

use crate::{
    actions::Action,
    error::{EaseError, EaseResult},
    to_host::connector::ConnectorHostService,
};

#[derive(Debug)]
pub enum ConnectorAction {
    PlaylistAbstracts(Vec<PlaylistAbstract>),
    Playlist(Playlist),
    Music(Music),
    Storages(Vec<Storage>),
}

pub struct Connector {
    state: Model<ConnectorState>,
    tasks: AsyncTasks,
}

impl Connector {
    pub fn new(cx: &mut AppBuilderContext) -> Self {
        Self {
            state: cx.model(),
            tasks: Default::default(),
        }
    }

    pub fn serve_url(&self, cx: &ViewModelContext, loc: StorageEntryLoc) -> String {
        let state= cx.model_get(&self.state);
        state.serve_url(loc)
    }

    fn init(&self, cx: &ViewModelContext, arg: ArgInitializeApp) -> EaseResult<()> {
        let host= ConnectorHostService::of(cx);
        host.init(arg)?;
        {
            let mut state = cx.model_mut(&self.state);
            state.port = host.port();
        }

        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            this.sync_storages(&cx).await?;
            this.sync_playlist_abstracts(&cx).await?;
            Ok(())
        });
        Ok(())
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
        self.request::<RemoveMusicsFromPlaylistMsg>(
            cx,
            ArgRemoveMusicFromPlaylist {
                playlist_id,
                music_id: id,
            },
        )
        .await?;
        self.sync_playlist(cx, playlist_id).await?;
        self.sync_playlist_abstracts(cx).await?;
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
        self.request::<CreatePlaylistMsg>(cx, arg).await?;
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

    pub async fn update_music_total_duration(
        &self,
        cx: &ViewModelContext,
        arg: ArgUpdateMusicDuration,
    ) -> EaseResult<()> {
        let id = arg.id;
        self.request::<UpdateMusicDurationMsg>(cx, arg).await?;
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
        self.sync_playlist_abstracts(cx).await?;
        self.sync_storages(cx).await?;
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
        Ok(())
    }

    async fn sync_music(&self, cx: &ViewModelContext, id: MusicId) -> EaseResult<()> {
        let music = self.get_music(cx, id).await?;
        if let Some(music) = music {
            cx.enqueue_emit::<_, EaseError>(Action::Connector(ConnectorAction::Music(music)));
        }
        Ok(())
    }

    async fn sync_playlist_abstracts(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let playlists = self.request::<GetAllPlaylistAbstractsMsg>(cx, ()).await?;
        cx.enqueue_emit::<_, EaseError>(Action::Connector(ConnectorAction::PlaylistAbstracts(
            playlists,
        )));
        Ok(())
    }

    async fn sync_playlist(&self, cx: &ViewModelContext, id: PlaylistId) -> EaseResult<()> {
        let playlist = self.request::<GetPlaylistMsg>(cx, id).await?;
        if let Some(playlist) = playlist {
            cx.enqueue_emit::<_, EaseError>(Action::Connector(ConnectorAction::Playlist(playlist)));
        }
        Ok(())
    }

    async fn sync_storages(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let storages = self.request::<ListStorageMsg>(cx, ()).await?;
        cx.enqueue_emit::<_, EaseError>(Action::Connector(ConnectorAction::Storages(storages)));
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

impl ViewModel<Action, EaseError> for Connector {
    fn on_event(&self, cx: &ViewModelContext, event: &Action) -> EaseResult<()> {
        match event {
            Action::Init(arg) => {
                self.init(cx, arg.clone())?;
            }
            _ => {}
        }
        Ok(())
    }
}
