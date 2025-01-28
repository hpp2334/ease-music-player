use crate::view_models::{
    main::{desktop_right_menu::DesktopRightMenuWidget, desktop_sidebar::DesktopSidebarWidget, MainBodyWidget},
    music::{
        control::MusicControlWidget, detail::MusicDetailWidget, lyric::MusicLyricWidget,
        time_to_pause::TimeToPauseWidget,
    },
    playlist::{
        create::PlaylistCreateWidget, detail::PlaylistDetailWidget, edit::PlaylistEditWidget,
        list::PlaylistListWidget,
    },
    storage::{import::StorageImportWidget, list::StorageListWidget, upsert::StorageUpsertWidget},
};

#[derive(Debug, Clone, uniffi::Enum)]
pub enum WidgetActionType {
    Click,
    RightClick {
        x: i32,
        y: i32,
    },
    ChangeText { text: String },
}
macro_rules! generate_widget {
    ($($widget_type:ident($widget_struct:ty)),*) => {
        #[derive(Debug, Clone, uniffi::Enum)]
        pub enum Widget {
            $($widget_type($widget_struct)),*
        }

        $(
            impl Into<Widget> for $widget_struct {
                fn into(self) -> Widget {
                    Widget::$widget_type(self)
                }
            }
        )*
    };
}

generate_widget!(
    MainBody(MainBodyWidget),
    DesktopSidebar(DesktopSidebarWidget),
    DesktopRightMenu(DesktopRightMenuWidget),
    MusicControl(MusicControlWidget),
    MusicLyric(MusicLyricWidget),
    MusicDetail(MusicDetailWidget),
    TimeToPause(TimeToPauseWidget),
    PlaylistDetail(PlaylistDetailWidget),
    PlaylistEdit(PlaylistEditWidget),
    PlaylistCreate(PlaylistCreateWidget),
    PlaylistList(PlaylistListWidget),
    StorageImport(StorageImportWidget),
    StorageList(StorageListWidget),
    StorageUpsert(StorageUpsertWidget)
);

#[derive(Debug, Clone, uniffi::Record)]
pub struct WidgetAction {
    pub widget: Widget,
    pub typ: WidgetActionType,
}
