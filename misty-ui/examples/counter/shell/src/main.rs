use std::path::Path;

use misty_ui::MistyUI;



fn main() {
    smol::block_on(async {
        MistyUI::new()
            .entry("../demo-ui/dist/main.js".into())
            .run()
            .await;
    });
}
