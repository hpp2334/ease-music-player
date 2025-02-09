use ease_client::DesktopRoutesKey;

#[derive(Debug)]
pub struct Router {
    items: DesktopRoutesKey,
}

impl Router {
    pub fn new() -> Self {
        Self {
            items: DesktopRoutesKey::Home,
        }
    }

    pub fn push(&mut self, key: DesktopRoutesKey) {
        self.items = key;
    }

    pub fn current(&self) -> DesktopRoutesKey {
        self.items.clone()
    }
}
