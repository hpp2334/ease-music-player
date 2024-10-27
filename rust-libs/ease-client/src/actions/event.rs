use ease_client_shared::backends::app::ArgInitializeApp;

use crate::view_models::{connector::ConnectorAction, music::{
    common::MusicCommonAction, control::MusicControlAction, time_to_pause::TimeToPauseAction,
}};

use super::WidgetAction;

#[derive(Debug)]
pub enum Action {
    Init(ArgInitializeApp),
    MusicCommon(MusicCommonAction),
    Connector(ConnectorAction),
    View(ViewAction)
}

#[derive(Debug, uniffi::Enum)]
pub enum ViewAction {
    MusicControl(MusicControlAction),
    TimeToPause(TimeToPauseAction),
    Widget(WidgetAction),
}