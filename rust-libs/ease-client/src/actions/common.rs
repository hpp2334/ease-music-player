#[derive(Debug, Clone)]
pub enum WidgetActionType {
    Click,
}

#[derive(Debug, Clone)]
pub enum Widget {
    // MusicControl
    MusicControlPlay,
    MusicControlPause,
    MusicControlPlayPrevious,
    MusicControlPlayNext,
    MusicControlStop,
    MusicControlPlaymode,
    // MusicLyric
    MusicLyricAdd,
    MusicLyricRemove,
}

#[derive(Debug, Clone)]
pub struct WidgetAction {
    pub widget: Widget,
    pub typ: WidgetActionType,
}
