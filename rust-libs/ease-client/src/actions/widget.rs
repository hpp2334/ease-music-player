use crate::view_models::{
    music::{control::MusicControlWidget, detail::MusicDetailWidget, lyric::MusicLyricWidget},
    playlist::{
        create::PlaylistCreateWidget, detail::PlaylistDetailWidget, edit::PlaylistEditWidget,
    },
    storage::{import::StorageImportWidget, list::StorageListWidget, upsert::StorageUpsertWidget},
};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum WidgetActionType {
    Click,
    ChangeText { text: String },
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum Widget {
    MusicControl(MusicControlWidget),
    MusicLyric(MusicLyricWidget),
    MusicDetail(MusicDetailWidget),
    PlaylistDetail(PlaylistDetailWidget),
    PlaylistEdit(PlaylistEditWidget),
    PlaylistCreate(PlaylistCreateWidget),
    StroageImport(StorageImportWidget),
    StorageList(StorageListWidget),
    StorageUpsert(StorageUpsertWidget),
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WidgetAction {
    pub widget: Widget,
    pub typ: WidgetActionType,
}
