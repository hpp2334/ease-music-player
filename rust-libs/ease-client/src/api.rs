use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use crate::modules::timer::to_host::{HostTimerService, TimerService};
use crate::{build_state_manager, build_view_manager, RootViewModelState};
use error::EaseResult;
use misty_vm::client::SingletonMistyClientPod;
use misty_vm::controllers::{ControllerRet, MistyController};
use misty_vm::resources::ResourceUpdateAction;
use misty_vm::services::MistyServiceManager;
use misty_vm::signals::MistySignal;
use tracing::subscriber::set_global_default;

use self::error::EaseError;

use super::modules::*;

static CLIENT: SingletonMistyClientPod<RootViewModelState> = SingletonMistyClientPod::new();

#[derive(Default, uniffi::Record)]
pub struct InvokeRet {
    pub view: Option<RootViewModelState>,
    pub resources: Vec<ResourceToHostAction>,
}

pub type ApiRet = EaseResult<InvokeRet>;

fn apply_controller_ret(
    ret: EaseResult<ControllerRet<RootViewModelState>>,
) -> EaseResult<InvokeRet> {
    if let Err(e) = ret {
        tracing::error!("{}", e);
        CLIENT.destroy();
        return Err(e);
    }
    let ret = ret.unwrap();

    let mut invoke_ret = InvokeRet {
        view: None,
        resources: Default::default(),
    };

    invoke_ret.view = ret.changed_view;
    for action in ret.changed_resources.into_iter() {
        let arg = match action {
            ResourceUpdateAction::Insert(id, buf) => ResourceToHostAction {
                id: *id,
                buf: Some(buf),
            },
            ResourceUpdateAction::Remove(id) => ResourceToHostAction { id: *id, buf: None },
        };
        invoke_ret.resources.push(arg);
    }
    Ok(invoke_ret)
}

fn call_controller<Controller, Arg>(controller: Controller, arg: Arg) -> EaseResult<InvokeRet>
where
    Controller: MistyController<Arg, EaseError>,
{
    let ret = CLIENT.call_controller(controller, arg);
    apply_controller_ret(ret)
}

struct ToHostMusicPlayerService;
impl IMusicPlayerService for ToHostMusicPlayerService {
    fn resume(&self) {
        todo!()
    }

    fn pause(&self) {
        todo!()
    }

    fn stop(&self) {
        todo!()
    }

    fn seek(&self, arg: u64) {
        todo!()
    }

    fn set_music_url(&self, url: String) {
        todo!()
    }
}

struct ToHostToastService;
impl IToastService for ToHostToastService {
    fn error(&self, msg: String) {
        todo!()
    }
}

fn create_log(dir: &str) -> std::fs::File {
    let p = std::path::Path::new(dir).join("latest.log");
    let _r = std::fs::remove_file(&p);
    let file = std::fs::File::create(&p).unwrap();
    file
}

#[cfg(target_os = "android")]
fn setup_subscriber(dir: &str) {
    use tracing_subscriber::layer::SubscriberExt;
    let log_file = create_log(dir);
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_writer(log_file)
        .with_ansi(false)
        .finish();
    let subscriber = subscriber.with(tracing_android::layer("com.ease_music_player").unwrap());
    set_global_default(subscriber).unwrap();
}

#[cfg(not(target_os = "android"))]
fn setup_subscriber(dir: &str) {
    let log_file = create_log(dir);
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .with_writer(log_file)
        .with_ansi(false)
        .finish();

    set_global_default(subscriber).unwrap();
}

fn initialize_trace(dir: &str) {
    static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);
    let is_init = IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst);
    std::env::set_var("RUST_BACKTRACE", "1");
    if !is_init {
        setup_subscriber(dir);
    }

    std::panic::set_hook(Box::new(|info| {
        // let stacktrace = std::backtrace::Backtrace::force_capture();

        // CLIENT.destroy();
        // try_invoke_sink(
        //     &REPORT_PANIC_SINK,
        //     ArgReportPanic {
        //         info: format!("{:?}", info),
        //         stack_trace: format!("{}", stacktrace),
        //     },
        // );
    }));

    tracing::info!("initialize log and backtrace");
}

#[derive(Debug, uniffi::Record)]
pub struct ResourceToHostAction {
    pub id: u64,
    pub buf: Option<Vec<u8>>,
}

#[uniffi::export]
pub fn initialize_client(arg: ArgInitializeApp) -> ApiRet {
    initialize_trace(&arg.app_document_dir);

    let view_manager = build_view_manager();
    let state_manager = build_state_manager();
    let service_manager = MistyServiceManager::builder()
        .add(MusicPlayerService::new(ToHostMusicPlayerService))
        .add(ToastService::new(ToHostToastService))
        .add(TimerService::new(HostTimerService))
        .build();
    CLIENT.reset();
    CLIENT.create(view_manager, state_manager, service_manager);
    CLIENT.on_signal(|signal| match signal {
        MistySignal::Schedule => todo!(),
    });

    let ret: InvokeRet = call_controller(controller_initialize_app, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn flush_schedule() -> ApiRet {
    let ret = CLIENT.flush_scheduled_tasks().unwrap();
    Ok(apply_controller_ret(Ok::<_, EaseError>(ret))?)
}

// API_GENERATE_MARKER
#[uniffi::export]
pub fn initialize_app(arg: ArgInitializeApp) -> ApiRet {
    let ret = call_controller(controller_initialize_app, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_storage_permission(arg: bool) -> ApiRet {
    let ret = call_controller(controller_update_storage_permission, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn play_music(arg: MusicId) -> ApiRet {
    let ret = call_controller(controller_play_music, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn pause_music() -> ApiRet {
    let ret = call_controller(controller_pause_music, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn resume_music() -> ApiRet {
    let ret = call_controller(controller_resume_music, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn stop_music() -> ApiRet {
    let ret = call_controller(controller_stop_music, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn seek_music(arg: ArgSeekMusic) -> ApiRet {
    let ret = call_controller(controller_seek_music, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn set_current_music_position_for_player_internal(arg: u64) -> ApiRet {
    let ret = call_controller(controller_set_current_music_position_for_player_internal, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_current_music_total_duration_for_player_internal(arg: u64) -> ApiRet {
    let ret = call_controller(controller_update_current_music_total_duration_for_player_internal, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_current_music_playing_for_player_internal(arg: bool) -> ApiRet {
    let ret = call_controller(controller_update_current_music_playing_for_player_internal, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn handle_play_music_event_for_player_internal(arg: PlayMusicEventType) -> ApiRet {
    let ret = call_controller(controller_handle_play_music_event_for_player_internal, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn play_next_music() -> ApiRet {
    let ret = call_controller(controller_play_next_music, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn play_previous_music() -> ApiRet {
    let ret = call_controller(controller_play_previous_music, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_music_playmode_to_next() -> ApiRet {
    let ret = call_controller(controller_update_music_playmode_to_next, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_time_to_pause(arg: u64) -> ApiRet {
    let ret = call_controller(controller_update_time_to_pause, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn remove_time_to_pause() -> ApiRet {
    let ret = call_controller(controller_remove_time_to_pause, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn prepare_import_lyric() -> ApiRet {
    let ret = call_controller(controller_prepare_import_lyric, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn remove_current_music_lyric() -> ApiRet {
    let ret = call_controller(controller_remove_current_music_lyric, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn change_to_current_music_playlist() -> ApiRet {
    let ret = call_controller(controller_change_to_current_music_playlist, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn prepare_edit_playlist(arg: PlaylistId) -> ApiRet {
    let ret = call_controller(controller_prepare_edit_playlist, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn finish_edit_playlist() -> ApiRet {
    let ret = call_controller(controller_finish_edit_playlist, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn prepare_edit_playlist_cover() -> ApiRet {
    let ret = call_controller(controller_prepare_edit_playlist_cover, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_edit_playlist_name(arg: String) -> ApiRet {
    let ret = call_controller(controller_update_edit_playlist_name, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn clear_edit_playlist_cover() -> ApiRet {
    let ret = call_controller(controller_clear_edit_playlist_cover, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn prepare_import_entries_in_current_playlist() -> ApiRet {
    let ret = call_controller(controller_prepare_import_entries_in_current_playlist, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn finish_create_playlist() -> ApiRet {
    let ret = call_controller(controller_finish_create_playlist, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn clear_create_playlist() -> ApiRet {
    let ret = call_controller(controller_clear_create_playlist, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn reset_create_playlist_full() -> ApiRet {
    let ret = call_controller(controller_reset_create_playlist_full, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn prepare_create_playlist_cover() -> ApiRet {
    let ret = call_controller(controller_prepare_create_playlist_cover, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn prepare_create_playlist_entries() -> ApiRet {
    let ret = call_controller(controller_prepare_create_playlist_entries, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn prepare_create_playlist() -> ApiRet {
    let ret = call_controller(controller_prepare_create_playlist, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_create_playlist_name(arg: String) -> ApiRet {
    let ret = call_controller(controller_update_create_playlist_name, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn clear_create_playlist_cover() -> ApiRet {
    let ret = call_controller(controller_clear_create_playlist_cover, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_create_playlist_mode(arg: CreatePlaylistMode) -> ApiRet {
    let ret = call_controller(controller_update_create_playlist_mode, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn change_current_playlist(arg: PlaylistId) -> ApiRet {
    let ret = call_controller(controller_change_current_playlist, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn remove_playlist(arg: PlaylistId) -> ApiRet {
    let ret = call_controller(controller_remove_playlist, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn remove_music_from_current_playlist(arg: MusicId) -> ApiRet {
    let ret = call_controller(controller_remove_music_from_current_playlist, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn play_all_musics() -> ApiRet {
    let ret = call_controller(controller_play_all_musics, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn clear_edit_playlist_state() -> ApiRet {
    let ret = call_controller(controller_clear_edit_playlist_state, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn update_root_subkey(arg: RootRouteSubKey) -> ApiRet {
    let ret = call_controller(controller_update_root_subkey, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn remove_storage(arg: StorageId) -> ApiRet {
    let ret = call_controller(controller_remove_storage, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn locate_entry(arg: String) -> ApiRet {
    let ret = call_controller(controller_locate_entry, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn select_entry(arg: String) -> ApiRet {
    let ret = call_controller(controller_select_entry, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn toggle_all_checked_entries() -> ApiRet {
    let ret = call_controller(controller_toggle_all_checked_entries, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn select_storage_in_import(arg: StorageId) -> ApiRet {
    let ret = call_controller(controller_select_storage_in_import, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn refresh_current_storage_in_import() -> ApiRet {
    let ret = call_controller(controller_refresh_current_storage_in_import, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn finish_selected_entries_in_import() -> ApiRet {
    let ret = call_controller(controller_finish_selected_entries_in_import, ())?;
    Ok(ret)
}

#[uniffi::export]
pub fn prepare_edit_storage(arg: Option<StorageId>) -> ApiRet {
    let ret = call_controller(controller_prepare_edit_storage, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn upsert_storage(arg: ArgUpsertStorage) -> ApiRet {
    let ret = call_controller(controller_upsert_storage, arg)?;
    Ok(ret)
}

#[uniffi::export]
pub fn test_connection(arg: ArgUpsertStorage) -> ApiRet {
    let ret = call_controller(controller_test_connection, arg)?;
    Ok(ret)
}

