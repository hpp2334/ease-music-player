use gpui::{AssetSource, SharedString};

pub struct Assets {}

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
        const DRAWABLES_PREFIX: &'static str = "drawables://";

        std::fs::read("assets/drawables/".to_string() + &path[DRAWABLES_PREFIX.len()..])
            .map(Into::into)
            .map_err(Into::into)
            .map(Some)
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        unimplemented!()
    }
}