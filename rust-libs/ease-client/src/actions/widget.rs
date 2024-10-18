use crate::view_models::{
    music::{control::MusicControlWidget, lyric::MusicLyricWidget},
    storage::import::StorageImportWidget,
};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum WidgetActionType {
    Click,
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum Widget {
    MusicControl(MusicControlWidget),
    MusicLyric(MusicLyricWidget),
    StroageImport(StorageImportWidget),
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WidgetAction {
    pub widget: Widget,
    pub typ: WidgetActionType,
}
