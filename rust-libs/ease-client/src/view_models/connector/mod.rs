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
    AppBuilderContext, AsyncTasks, AsyncViewModelContext, BoxFuture, IToHost, Model, ViewModel,
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

    pub fn storage_path(&self, cx: &ViewModelContext) -> String {
        ConnectorHostService::of(cx).storage_path()
    }

    fn init(&self, cx: &ViewModelContext) -> EaseResult<()> {
        let host = ConnectorHostService::of(cx);
        let handle = host.connect(Arc::new(ConnectorNotifier { cx: cx.weak() }));
        {
            let mut state = cx.model_mut(&self.state);
            state.connector_handle = handle;
        }

        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            this.request::<OnConnectMsg>(&cx, ()).await?;
            cx.enqueue_emit(Action::VsLoaded);
            Ok(())
        });
        Ok(())
    }

    pub(crate) fn destroy(&self, cx: &ViewModelContext) {
        let handle = cx.model_get(&self.state).connector_handle;
        ConnectorHostService::of(cx).disconnect(handle);
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

    fn after_request<S: IMessage>(&self, cx: &ViewModelContext) {
        let this = Self::of(cx);
        cx.spawn::<_, _, EaseError>(&self.tasks, move |cx| async move {
            if S::CODE == RemoveStorageMsg::CODE {
                this.sync_current_playlist(&cx).await?;
            }
            Ok(())
        });
    }

    pub(crate) async fn request<S: IMessage>(
        &self,
        cx: &ViewModelContext,
        arg: <S as IMessage>::Argument,
    ) -> EaseResult<<S as IMessage>::Return> {
        let connector = ConnectorHostService::of(cx);
        let ret = cx
            .spawn_background::<_, EaseError>(async move {
                let arg = encode_message_payload(arg);
                let ret = connector
                    .request(MessagePayload {
                        code: S::CODE,
                        payload: arg,
                    })
                    .await?;
                let ret = decode_message_payload(ret.payload);

                Ok::<_, EaseError>(ret)
            })
            .await?;
        self.after_request::<S>(cx);
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
