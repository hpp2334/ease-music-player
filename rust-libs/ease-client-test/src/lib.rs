use std::sync::atomic::{AtomicBool, AtomicUsize};

use std::sync::Arc;
use std::time::Duration;

use ease_client::{
    build_client, Action, IRouterService, IToastService, PlaylistCreateWidget,
    PlaylistDetailWidget, PlaylistListWidget, RootViewModelState, RoutesKey, StorageImportWidget,
    StorageListWidget, StorageUpsertWidget, ViewAction, Widget, WidgetAction, WidgetActionType,
};
use ease_client_shared::backends::app::ArgInitializeApp;
use ease_client_shared::backends::music::MusicId;
use ease_client_shared::backends::playlist::PlaylistId;
use ease_client_shared::backends::storage::{StorageId, StorageType};
use ease_client_shared::uis::playlist::CreatePlaylistMode;
use fake_player::*;
pub use fake_server::ReqInteceptor;
use fake_server::*;
use misty_vm::AppPod;
use misty_vm_test::AsyncRuntime;
use view_state::ViewStateServiceRef;

mod fake_player;
mod fake_server;
mod view_state;

pub struct TestApp {
    app: AppPod,
    server: FakeServerRef,
    player: FakeMusicPlayerRef,
    view_state: ViewStateServiceRef,
    async_runtime: AsyncRuntime,
    last_wait_req_session: AtomicUsize,
}

#[derive(Debug, Clone, Copy)]
pub enum PresetDepth {
    Storage,
    Playlist,
    Music,
}

struct FakeToastServiceImpl;
impl IToastService for FakeToastServiceImpl {
    fn error(&self, _msg: String) {
        // noop
    }
}

static SETUP_SUBCRIBER_ONCE: AtomicBool = AtomicBool::new(false);

static SCHEMA_VERSION: u32 = 1;

fn setup_subscriber() {
    let has_setup = SETUP_SUBCRIBER_ONCE.swap(true, std::sync::atomic::Ordering::Relaxed);
    if has_setup {
        return;
    }

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

struct FakeRouterService;
impl IRouterService for FakeRouterService {
    fn naviagate(&self, _key: RoutesKey) {}
    fn pop(&self) {}
}

impl TestApp {
    pub async fn new(test_dir: &str, cleanup: bool) -> Self {
        let test_dir = if test_dir.ends_with("/") {
            test_dir.to_string()
        } else {
            test_dir.to_string() + "/"
        };
        std::env::set_var("RUST_BACKTRACE", "1");
        setup_subscriber();

        let mut cwd = std::env::current_dir().unwrap();
        if cwd.ends_with("rust-libs") {
            cwd = cwd.join(std::path::Path::new("ease-client-test"));
        }
        std::env::set_current_dir(cwd).unwrap();

        if cleanup {
            if std::fs::metadata(&test_dir).is_ok() {
                std::fs::remove_dir_all(&test_dir).unwrap();
            }
        }
        if std::fs::metadata(&test_dir).is_err() {
            std::fs::create_dir_all(&test_dir).unwrap();
        }

        let pod = AppPod::new();
        let player = FakeMusicPlayerRef::new(pod.clone());
        let async_runtime = AsyncRuntime::new();
        let view_state = ViewStateServiceRef::new();
        let app = build_client(
            Arc::new(FakeRouterService),
            Arc::new(player.clone()),
            Arc::new(FakeToastServiceImpl),
            Arc::new(view_state.clone()),
            async_runtime.clone(),
        );
        async_runtime.bind_app(app.clone());
        pod.set(app.clone());

        let storage_path = {
            let dir = std::env::current_dir().unwrap();
            let dir = dir.to_string_lossy().to_string();
            if std::env::consts::OS == "windows" {
                dir.replace('\\', "/")
            } else {
                dir
            }
        };

        app.emit(Action::Init(ArgInitializeApp {
            app_document_dir: test_dir.to_string(),
            schema_version: SCHEMA_VERSION,
            storage_path,
        }));

        let ret = Self {
            app: pod,
            server: FakeServerRef::setup("test-files"),
            player,
            async_runtime,
            view_state,
            last_wait_req_session: Default::default(),
        };
        ret.wait_network().await;
        ret
    }

    pub fn emit(&self, action: Action) {
        self.app.get().emit(action);
        self.player.flush_player_events();
    }

    pub fn dispatch_click(&self, widget: impl Into<Widget>) {
        self.emit(Action::View(ViewAction::Widget(WidgetAction {
            widget: widget.into(),
            typ: WidgetActionType::Click,
        })));
    }

    pub fn dispatch_change_text(&self, widget: impl Into<Widget>, text: impl ToString) {
        self.emit(Action::View(ViewAction::Widget(WidgetAction {
            widget: widget.into(),
            typ: WidgetActionType::ChangeText {
                text: text.to_string(),
            },
        })));
    }

    async fn create_empty_playlist(&self) {
        self.dispatch_click(PlaylistListWidget::Add);
        self.dispatch_click(PlaylistCreateWidget::Tab {
            value: CreatePlaylistMode::Empty,
        });
        self.dispatch_change_text(PlaylistCreateWidget::Name, "Default Playlist");
        self.dispatch_click(PlaylistCreateWidget::FinishCreate);
        self.wait_network().await;
    }

    pub async fn setup_preset(&mut self, depth: PresetDepth) {
        self.dispatch_click(StorageListWidget::Create);
        self.dispatch_change_text(StorageUpsertWidget::Address, self.server.addr());
        self.dispatch_change_text(StorageUpsertWidget::Alias, "Temp");
        self.dispatch_click(StorageUpsertWidget::Type {
            value: StorageType::Webdav,
        });
        self.dispatch_click(StorageUpsertWidget::Finish);
        self.wait_network().await;

        if depth as i32 >= PresetDepth::Playlist as i32 {
            self.create_empty_playlist().await;
        }
        if depth as i32 >= PresetDepth::Music as i32 {
            let playlist_id = self.get_first_playlist_id_from_latest_state();
            self.dispatch_click(PlaylistListWidget::Item { id: playlist_id });
            let storage_id = self.get_first_storage_id_from_latest_state();
            self.wait_network().await;
            self.dispatch_click(PlaylistDetailWidget::Import);
            self.dispatch_click(StorageImportWidget::StorageItem { id: storage_id });
            for _ in 0..10 {
                self.wait_network().await;
                let state = self.latest_state();
                let entries = state.current_storage_entries.unwrap();
                if !entries.entries.is_empty() {
                    break;
                }
                tracing::info!("wait storage entries to be not empty");
            }
            let state = self.latest_state();
            let entries = state.current_storage_entries.unwrap();
            self.dispatch_click(StorageImportWidget::StorageEntry {
                path: entries.entries[4].path.clone(),
            });
            self.dispatch_click(StorageImportWidget::StorageEntry {
                path: entries.entries[5].path.clone(),
            });
            self.dispatch_click(StorageImportWidget::Import);
            self.wait_network().await;
        }
    }

    pub fn latest_state(&self) -> RootViewModelState {
        self.view_state.state()
    }

    pub fn get_first_storage_id_from_latest_state(&self) -> StorageId {
        let state = self.latest_state();
        let storage_id = state.storage_list.as_ref().unwrap().items[0]
            .storage_id
            .clone();
        storage_id
    }

    pub fn get_last_storage_id_from_latest_state(&self) -> StorageId {
        let state = self.latest_state();
        let storage_id = state
            .storage_list
            .as_ref()
            .unwrap()
            .items
            .last()
            .unwrap()
            .storage_id
            .clone();
        storage_id
    }

    pub fn get_first_playlist_id_from_latest_state(&self) -> PlaylistId {
        let state = self.latest_state();
        let playlist_id = state.playlist_list.unwrap().playlist_list[0].id.clone();
        playlist_id
    }

    pub fn get_first_music_id_from_latest_state(&self) -> MusicId {
        let state = self.latest_state();
        let music_id = state.current_playlist.as_ref().unwrap().items[0].id.clone();
        music_id
    }

    pub fn get_second_music_id_from_latest_state(&self) -> MusicId {
        let state = self.latest_state();
        let music_id = state.current_playlist.as_ref().unwrap().items[1].id.clone();
        music_id
    }

    pub fn get_lastest_bytes(&self) -> Vec<u8> {
        self.player.last_bytes()
    }

    pub async fn advance_timer(&self, mut duration_s: u64) {
        self.wait_network().await;
        loop {
            let t = duration_s.min(1);
            duration_s -= t;

            if t == 0 {
                break;
            }

            self.advance_timer_impl(t);
            self.wait_network().await;
        }
    }

    fn advance_timer_impl(&self, duration_s: u64) {
        let duration = Duration::from_secs(duration_s);
        self.player.advance(duration);
        self.async_runtime.advance(duration);
    }

    pub async fn wait_network(&self) {
        for _ in 0..3 {
            self.wait(20).await;
        }
    }

    pub fn set_inteceptor_req(&self, v: Option<ReqInteceptor>) {
        self.server.set_inteceptor_req(v);
    }

    pub async fn load_resource(&self, url: impl ToString) -> Vec<u8> {
        self.server.load_resource(url).await
    }

    async fn wait(&self, mut ms: u64) {
        tokio::time::sleep(Duration::from_millis(0)).await;
        loop {
            let t = ms.min(4);
            ms -= t;

            if t == 0 {
                break;
            }
            self.advance_timer_impl(0);
            self.player.flush_player_events();
            tokio::time::sleep(Duration::from_millis(t)).await;
        }
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {}
}
