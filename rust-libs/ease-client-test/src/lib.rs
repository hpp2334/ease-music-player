use std::sync::atomic::AtomicBool;

use std::time::Duration;

use ease_client::modules::timer::to_host::TimerService;
use ease_client::{
    build_state_manager, build_view_manager, modules::*, MistyController, MistyResourceId,
    MistyServiceManager, RootViewModelState,
};

use fake_player::*;
pub use fake_server::ReqInteceptor;
use fake_server::*;
use fake_timer::*;
use misty_vm_test::TestAppContainer;

mod fake_player;
mod fake_server;
mod fake_timer;
mod rt;

pub struct TestApp {
    app: misty_vm_test::TestApp<RootViewModelState>,
    server: FakeServerRef,
    player: FakeMusicPlayerRef,
    timer: FakeTimerServiceRef,
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
    let has_setup = SETUP_SUBCRIBER_ONCE.swap(true, std::sync::atomic::Ordering::SeqCst);
    if has_setup {
        return;
    }

    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .finish();
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

impl TestApp {
    pub fn new(test_dir: &str, cleanup: bool) -> Self {
        let test_dir = if test_dir.ends_with("/") {
            test_dir.to_string()
        } else {
            test_dir.to_string() + "/"
        };
        // std::env::set_var("RUST_BACKTRACE", "1");
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

        let app_container = TestAppContainer::<RootViewModelState>::new(|changed, state| {
            state.merge_from(&changed);
        });
        let mut timer = FakeTimerServiceRef::new();
        let player = FakeMusicPlayerRef::new(app_container.clone());
        timer.bind_music_player(player.clone());
        let service_manager = MistyServiceManager::builder()
            .add(MusicPlayerService::new(player.clone()))
            .add(ToastService::new(FakeToastServiceImpl))
            .add(TimerService::new(timer.clone()))
            .build();
        let state_manager = build_state_manager();
        let view_manager = build_view_manager();
        let inner = misty_vm_test::TestApp::new(
            view_manager,
            service_manager,
            state_manager,
            app_container,
        );

        let storage_path = {
            let dir = std::env::current_dir().unwrap();
            let dir = dir.to_string_lossy().to_string();
            if std::env::consts::OS == "windows" {
                dir.replace('\\', "/")
            } else {
                dir
            }
        };

        inner.app().call_controller(
            controller_initialize_app,
            ArgInitializeApp {
                app_document_dir: test_dir.to_string(),
                schema_version: SCHEMA_VERSION,
                storage_path,
            },
        );

        TestApp {
            app: inner,
            server: FakeServerRef::setup("test-files"),
            player,
            timer,
        }
    }

    fn create_empty_playlist(&self) {
        self.call_controller(controller_prepare_create_playlist, ());
        self.call_controller(
            controller_update_create_playlist_name,
            "Default Playlist".to_string(),
        );
        self.call_controller(controller_finish_create_playlist, ());
    }

    pub fn setup_preset(&mut self, depth: PresetDepth) {
        self.call_controller(
            controller_upsert_storage,
            ArgUpsertStorage {
                id: None,
                addr: self.server.addr(),
                alias: Some("Temp".to_string()),
                username: Default::default(),
                password: Default::default(),
                is_anonymous: true,
                typ: StorageType::Webdav,
            },
        );
        if depth as i32 >= PresetDepth::Playlist as i32 {
            self.create_empty_playlist();
        }
        if depth as i32 >= PresetDepth::Music as i32 {
            let playlist_id = self.get_first_playlist_id_from_latest_state();
            self.call_controller(controller_change_current_playlist, playlist_id);
            let storage_id = self.get_first_storage_id_from_latest_state();
            self.call_controller(controller_prepare_import_entries_in_current_playlist, ());
            self.call_controller(controller_select_storage_in_import, storage_id);
            self.wait_network();
            let state = self.latest_state();
            let entries = state.current_storage_entries.unwrap();
            self.call_controller(controller_select_entry, entries.entries[4].path.clone());
            self.call_controller(controller_select_entry, entries.entries[5].path.clone());
            self.call_controller(controller_finish_selected_entries_in_import, ());
        }
    }

    pub fn latest_state(&self) -> RootViewModelState {
        self.wait(100);
        self.app.state()
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

    pub fn call_controller<Arg, E: std::fmt::Debug>(
        &self,
        controller: impl MistyController<Arg, E>,
        arg: Arg,
    ) {
        self.app.app().call_controller(controller, arg);
    }

    pub fn advance_timer(&self, duration_s: u64) {
        self.timer.advance_timer(duration_s);
    }

    pub fn wait_network(&self) {
        self.wait(200);
    }

    pub fn set_inteceptor_req(&self, v: Option<ReqInteceptor>) {
        self.server.set_inteceptor_req(v);
    }

    pub fn load_resource(&self, id: u64) -> Vec<u8> {
        self.app
            .app()
            .get_resource(MistyResourceId::wrap(id))
            .unwrap()
    }

    fn wait(&self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {}
}
