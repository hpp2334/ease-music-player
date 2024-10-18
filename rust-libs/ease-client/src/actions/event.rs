use crate::view_models::music::{common::MusicCommonAction, control::MusicControlAction};

use super::WidgetAction;

pub enum Action {
    MusicCommon(MusicCommonAction),
    MusicControl(MusicControlAction),
    Widget(WidgetAction),
}
