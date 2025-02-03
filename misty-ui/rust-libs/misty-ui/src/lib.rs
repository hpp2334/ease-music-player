use std::rc::Rc;



pub struct MistyUI {
    js_cx: deno_core::JsRuntime,
    entry_path: String,
}

impl MistyUI {
    pub fn new() -> Self {
        Self {
            js_cx: deno_core::JsRuntime::new(deno_core::RuntimeOptions {
                module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
                ..Default::default()
            }),
            entry_path: Default::default(),
        }
    }

    pub fn entry(mut self, entry_path: &str) -> Self {
        self.entry_path = entry_path.to_string();
        self
    }

    pub async fn run(mut self) {
        let cwd = std::env::current_dir().unwrap();
        let entry_url = deno_core::resolve_path(self.entry_path.as_str(), &cwd).unwrap();
        println!("p: {}", entry_url);
        let mod_id = self.js_cx.load_main_es_module(&entry_url).await.unwrap();
        let fut = self.js_cx.mod_evaluate(mod_id);
        self.js_cx.run_event_loop(Default::default()).await.unwrap();
        fut.await.unwrap();
    }
}
