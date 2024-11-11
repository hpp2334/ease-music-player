use ease_client_shared::backends::app::ArgInitializeApp;

use crate::view_models::{
    connector::ConnectorAction,
    main::{router::RouterAction, MainAction},
    music::{
        common::MusicCommonAction,
        control::{MusicControlAction, PlayerEvent},
        time_to_pause::TimeToPauseAction,
    },
    storage::import::StorageImportAction,
};

use super::WidgetAction;

#[derive(Debug)]
pub enum Action {
    Init(ArgInitializeApp),
    MusicCommon(MusicCommonAction),
    Connector(ConnectorAction),
    View(ViewAction),
}

#[derive(Debug, uniffi::Enum)]
pub enum ViewAction {
    MusicControl(MusicControlAction),
    StorageImport(StorageImportAction),
    TimeToPause(TimeToPauseAction),
    Player(PlayerEvent),
    Router(RouterAction),
    Main(MainAction),
    Widget(WidgetAction),
}
