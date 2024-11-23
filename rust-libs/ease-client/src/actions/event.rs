use ease_client_shared::backends::connector::ConnectorAction;

use crate::view_models::{
    main::{router::RouterAction, MainAction},
    music::{
        common::MusicCommonAction, control::MusicControlAction, time_to_pause::TimeToPauseAction,
    },
    storage::import::StorageImportAction,
};

use super::WidgetAction;

#[derive(Debug)]
pub enum Action {
    Init,
    Destroy,
    VsLoaded,
    MusicCommon(MusicCommonAction),
    Connector(ConnectorAction),
    View(ViewAction),
}

#[derive(Debug, uniffi::Enum)]
pub enum ViewAction {
    MusicControl(MusicControlAction),
    StorageImport(StorageImportAction),
    TimeToPause(TimeToPauseAction),
    Router(RouterAction),
    Main(MainAction),
    Widget(WidgetAction),
}
