use std::{cell::RefCell, rc::Rc, sync::Arc};

use base64::{engine::general_purpose::URL_SAFE, Engine as _};
use ease_client_backend::{error::BResult, Backend};
use ease_client_shared::backends::{
    app::ArgInitializeApp, message::{decode_message_payload, encode_message_payload, IMessage, MessagePayload}, music::{ArgUpdateMusicLyric, GetMusicMsg, Music, MusicId, UpdateMusicLyricMsg}, playlist::{
        AddMusicsToPlaylistMsg, ArgAddMusicsToPlaylist, ArgCreatePlaylist, ArgRemoveMusicFromPlaylist, ArgUpdatePlaylist, CreatePlaylistMsg, GetAllPlaylistAbstractsMsg, GetPlaylistMsg, Playlist, PlaylistAbstract, PlaylistId, RemoveMusicsFromPlaylistMsg, RemovePlaylistMsg, UpdatePlaylistMsg
    }, storage::{
        ArgUpsertStorage, GetStorageMsg, ListStorageEntryChildrenMsg, ListStorageEntryChildrenResp, ListStorageMsg, RemoveStorageMsg, Storage, StorageConnectionTestResult, StorageEntryLoc, StorageId, TestStorageMsg, UpsertStorageMsg
    }
};
use misty_vm::{AppBuilderContext, IToHost, ViewModel, ViewModelContext};

use crate::{
    actions::{event::ViewAction, Action},
    error::{EaseError, EaseResult},
    to_host::connector::ConnectorHostService,
};


#[derive(Debug)]
pub enum ConnectorAction {
    PlaylistAbstracts(Vec<PlaylistAbstract>),
    Playlist(Playlist),
    Music(Music),
    Stroages(Vec<Storage>),
}


pub struct Connector {}

impl Connector {
    pub fn new(_cx: &mut AppBuilderContext) -> Self {
        Self {}
    }

    pub fn serve_url(&self, cx: &ViewModelContext, loc: StorageEntryLoc) -> String {
        let port = ConnectorHostService::of(cx).port();
        let sp = URL_SAFE.encode(loc.path);
        let id: i64 = *loc.storage_id.as_ref();
        format!("http://127.0.0.1:{}/asset/{}?sp={}", port, id, sp)
    }

    fn init(&self, cx: &ViewModelContext, arg: ArgInitializeApp) -> EaseResult<()> {
        ConnectorHostService::of(cx).init(arg)?;
        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(move |cx| async move {
            this.sync_storages(&cx).await?;
            this.sync_playlist_abstracts(&cx).await?;
            Ok(())
        });
        Ok(())
    }

    pub async fn get_music(&self, cx: &ViewModelContext, id: MusicId) -> EaseResult<Option<Music>> {
        self.request::<GetMusicMsg>(cx, id).await
    }

    pub async fn get_playlist(&self, cx: &ViewModelContext, id: PlaylistId) -> EaseResult<Option<Playlist>> {
        self.request::<GetPlaylistMsg>(cx, id).await
    }

    pub async fn remove_music(&self, cx: &ViewModelContext, id: MusicId, playlist_id: PlaylistId) -> EaseResult<()> {
        self.request::<RemoveMusicsFromPlaylistMsg>(cx, ArgRemoveMusicFromPlaylist {
            playlist_id,
            music_id: id,
        }).await?;
        self.sync_playlist(cx, playlist_id).await?;
        self.sync_playlist_abstracts(cx).await?;
        Ok(())
    }

    pub async fn update_playlist(&self, cx: &ViewModelContext, arg: ArgUpdatePlaylist) -> EaseResult<()> {
        let id = arg.id;
        self.request::<UpdatePlaylistMsg>(cx, arg).await?;
        self.sync_playlist(cx, id).await?;
        self.sync_playlist_abstracts(cx).await?;
        Ok(())
    }

    pub async fn create_playlist(&self, cx: &ViewModelContext, arg: ArgCreatePlaylist) -> EaseResult<()> {
        self.request::<CreatePlaylistMsg>(cx, arg).await?;
        self.sync_playlist_abstracts(cx).await?;
        Ok(())
    }

    pub async fn remove_playlist(&self, cx: &ViewModelContext, id: PlaylistId) -> EaseResult<()> {
        self.request::<RemovePlaylistMsg>(cx, id).await?;
        self.sync_playlist_abstracts(cx).await?;
        Ok(())
    }

    pub async fn add_musics_to_playlist(&self, cx: &ViewModelContext, arg: ArgAddMusicsToPlaylist) -> EaseResult<()> {
        let id = arg.id;
        self.request::<AddMusicsToPlaylistMsg>(cx, arg).await?;
        self.sync_playlist(cx, id).await?;
        self.sync_playlist_abstracts(cx).await?;
        Ok(())
    }

    pub async fn update_music_lyric(&self, cx: &ViewModelContext, arg: ArgUpdateMusicLyric) -> EaseResult<()> {
        let id = arg.id;
        self.request::<UpdateMusicLyricMsg>(cx, arg).await?;
        self.sync_music(cx, id).await?;
        Ok(())
    }

    pub async fn list_storage_entry_children(
        &self, cx: &ViewModelContext,
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
        &self, cx: &ViewModelContext,
        arg: ArgUpsertStorage,
    ) -> EaseResult<StorageConnectionTestResult> {
        self.request::<TestStorageMsg>(cx, arg).await
    }

    pub async fn upsert_storage(&self, cx: &ViewModelContext, arg: ArgUpsertStorage) -> EaseResult<()> {
        self.request::<UpsertStorageMsg>(cx, arg).await?;
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
        cx.enqueue_emit(Action::Connector(ConnectorAction::PlaylistAbstracts(playlists)));
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
        let storages= self.request::<ListStorageMsg>(cx, ()).await?;
        cx.enqueue_emit(Action::Connector(ConnectorAction::Stroages(storages)));
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
            Action::View(action) => {
                match action {
                    ViewAction::Init(arg) => {
                        self.init(cx, arg.clone())?;
                    },
                    _ => {}
                }
            },
            _ => {}
        }
        Ok(())
    }
}
