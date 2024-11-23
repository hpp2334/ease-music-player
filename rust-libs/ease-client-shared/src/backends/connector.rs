use serde::{Deserialize, Serialize};

use super::{
    music::{Music, MusicId},
    player::ConnectorPlayerAction,
    playlist::{Playlist, PlaylistAbstract},
    storage::Storage,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ConnectorAction {
    PlaylistAbstracts(Vec<PlaylistAbstract>),
    Playlist(Playlist),
    Music(Music),
    Storages(Vec<Storage>),
    Player(ConnectorPlayerAction),
    MusicTotalDurationChanged(MusicId),
    MusicCoverChanged(MusicId),
}

pub trait IConnectorNotifier: Send + Sync + 'static {
    fn notify(&self, action: ConnectorAction);
}
