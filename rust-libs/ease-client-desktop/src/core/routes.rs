use ease_client::DesktopRoutesKey;

#[derive(Debug, Clone)]
pub struct Routes {
    items: Vec<DesktopRoutesKey>,
}

pub struct Router {
    routes: Routes,
    dirty: bool,
}

impl Routes {
    pub fn new() -> Self {
        Self {
            items: Default::default(),
        }
    }

    pub fn current(&self) -> DesktopRoutesKey {
        self.items.last().cloned().unwrap_or(DesktopRoutesKey::Home)
    }
}

impl Router {
    pub fn new() -> Self {
        Self {
            routes: Routes {
                items: Default::default(),
            },
            dirty: false,
        }
    }

    pub fn push(&mut self, key: DesktopRoutesKey) {
        let last = self.routes.items.last();

        self.dirty = true;
        self.routes.items.push(key);
    }

    pub fn pop(&mut self) {
        // self.dirty = true;
        // self.routes.items.pop();
    }

    pub fn get_changed_routes(&mut self) -> Option<Routes> {
        if self.dirty {
            self.dirty = false;
            Some(self.routes.clone())
        } else {
            None
        }
    }
}
