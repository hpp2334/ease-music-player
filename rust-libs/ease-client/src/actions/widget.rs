use crate::view_models::{
    main::MainBodyWidget, music::{
        control::MusicControlWidget, detail::MusicDetailWidget, lyric::MusicLyricWidget,
        time_to_pause::TimeToPauseWidget,
    }, playlist::{
        create::PlaylistCreateWidget, detail::PlaylistDetailWidget, edit::PlaylistEditWidget, list::PlaylistListWidget,
    }, storage::{import::StorageImportWidget, list::StorageListWidget, upsert::StorageUpsertWidget}
};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum WidgetActionType {
    Click,
    ChangeText { text: String },
}

#[derive(Debug, Clone, uniffi::Enum)]
pub enum Widget {
    MainBody(MainBodyWidget),
    MusicControl(MusicControlWidget),
    MusicLyric(MusicLyricWidget),
    MusicDetail(MusicDetailWidget),
    TimeToPause(TimeToPauseWidget),
    PlaylistDetail(PlaylistDetailWidget),
    PlaylistEdit(PlaylistEditWidget),
    PlaylistCreate(PlaylistCreateWidget),
    PlaylistList(PlaylistListWidget),
    StroageImport(StorageImportWidget),
    StorageList(StorageListWidget),
    StorageUpsert(StorageUpsertWidget),
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WidgetAction {
    pub widget: Widget,
    pub typ: WidgetActionType,
}
