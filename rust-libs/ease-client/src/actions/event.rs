use crate::view_models::music::{
    common::MusicCommonAction, control::MusicControlAction, time_to_pause::TimeToPauseAction,
};

use super::WidgetAction;

#[derive(Debug, uniffi::Enum)]
pub enum Action {
    MusicCommon(MusicCommonAction),
    MusicControl(MusicControlAction),
    TimeToPause(TimeToPauseAction),
    Widget(WidgetAction),
}
