use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use crate::modules::timer::to_host::{HostTimerService, TimerService};
use crate::{build_state_manager, build_view_manager, RootViewModelState};
use flutter_rust_bridge::rust2dart::IntoIntoDart;
use flutter_rust_bridge::{IntoDart, StreamSink, SyncReturn};
use misty_vm::client::SingletonMistyClientPod;
use misty_vm::controllers::{ControllerRet, MistyController};
use misty_vm::resources::ResourceUpdateAction;
use misty_vm::services::MistyServiceManager;
use misty_vm::signals::MistySignal;
use tracing::subscriber::set_global_default;

use self::error::EaseError;

use super::modules::*;

static CLIENT: SingletonMistyClientPod<RootViewModelState> = SingletonMistyClientPod::new();

#[derive(Default)]
pub struct InvokeRet {
    pub view: Option<RootViewModelState>,
    pub resources: Vec<ResourceToHostAction>,
}

pub type ApiRet = Result<SyncReturn<InvokeRet>, String>;

fn apply_controller_ret<E>(
    ret: Result<ControllerRet<RootViewModelState>, E>,
) -> Result<InvokeRet, String>
where
    E: std::error::Error + Into<anyhow::Error> + Send,
{
    if let Err(e) = ret {
        tracing::error!("{}", e);
        CLIENT.destroy();
        try_invoke_sink(
            &REPORT_PANIC_SINK,
            ArgReportPanic {
                info: format!("call controller error"),
                stack_trace: format!("{}", e),
            },
        );
        return Err(format!("{}", e));
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

fn call_controller<Controller, Arg, E>(
    controller: Controller,
    arg: Arg,
) -> Result<InvokeRet, String>
where
    Controller: MistyController<Arg, E>,
    E: std::error::Error + Sync + Send + 'static,
{
    let ret = CLIENT.call_controller(controller, arg);
    apply_controller_ret(ret)
}

macro_rules! define_to_host_controller {
    ($sink:ident,$arg:ty,$func:ident) => {
        static $sink: once_cell::sync::Lazy<Mutex<Option<StreamSink<$arg>>>> =
            once_cell::sync::Lazy::new(|| Default::default());
        pub fn $func(sink: StreamSink<$arg>) {
            tracing::info!("sink {} binded", stringify!($sink));
            let mut guard = $sink.lock().unwrap();
            *guard = Some(sink);
        }
    };
}

fn invoke_sink<D, T>(sink: &once_cell::sync::Lazy<Mutex<Option<StreamSink<T>>>>, arg: T)
where
    D: IntoDart,
    T: IntoIntoDart<D>,
{
    sink.lock().unwrap().as_ref().unwrap().add(arg);
}

fn try_invoke_sink<D, T>(sink: &once_cell::sync::Lazy<Mutex<Option<StreamSink<T>>>>, arg: T)
where
    D: IntoDart,
    T: IntoIntoDart<D>,
{
    let sink = sink.lock().unwrap();
    if let Some(sink) = sink.as_ref() {
        sink.add(arg);
    }
}

pub struct ArgReportPanic {
    pub info: String,
    pub stack_trace: String,
}
define_to_host_controller!(REPORT_PANIC_SINK, ArgReportPanic, init_bind_report_panic);
define_to_host_controller!(NOTIFY_SCHEDULE, (), init_notify_schedule);
define_to_host_controller!(RESUME_SINK, (), init_bind_resume_music);
define_to_host_controller!(PAUSE_SINK, (), init_bind_pause_music);
define_to_host_controller!(STOP_SINK, (), init_bind_stop_music);
define_to_host_controller!(SEEK_SINK, u64, init_seek_music);
define_to_host_controller!(SET_MUSIC_URL_SINK, String, set_music_url);
struct ToHostMusicPlayerService;
impl IMusicPlayerService for ToHostMusicPlayerService {
    fn resume(&self) {
        invoke_sink(&RESUME_SINK, ());
    }

    fn pause(&self) {
        invoke_sink(&PAUSE_SINK, ());
    }

    fn stop(&self) {
        invoke_sink(&STOP_SINK, ());
    }

    fn seek(&self, arg: u64) {
        invoke_sink(&SEEK_SINK, arg);
    }

    fn set_music_url(&self, url: String) {
        invoke_sink(&SET_MUSIC_URL_SINK, url);
    }
}

define_to_host_controller!(TOAST_ERROR_SINK, String, bind_toast_error);
struct ToHostToastService;
impl IToastService for ToHostToastService {
    fn error(&self, msg: String) {
        invoke_sink(&TOAST_ERROR_SINK, msg);
    }
}

fn create_log(dir: &str) -> std::fs::File {
    let p = std::path::Path::new(dir).join("latest.log");
    let _r = std::fs::remove_file(&p);
    let file = std::fs::File::create(&p).unwrap();
    file
}

#[cfg(not(target_os = "windows"))]
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

#[cfg(target_os = "windows")]
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
        let stacktrace = std::backtrace::Backtrace::force_capture();

        CLIENT.destroy();
        try_invoke_sink(
            &REPORT_PANIC_SINK,
            ArgReportPanic {
                info: format!("{:?}", info),
                stack_trace: format!("{}", stacktrace),
            },
        );
    }));

    tracing::info!("initialize log and backtrace");
}

pub struct ResourceToHostAction {
    pub id: u64,
    pub buf: Option<Vec<u8>>,
}

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
        MistySignal::Schedule => invoke_sink(&NOTIFY_SCHEDULE, ()),
    });

    let ret = call_controller(controller_initialize_app, arg)?;
    Ok(SyncReturn(ret))
}

pub fn flush_schedule() -> ApiRet {
    let ret = CLIENT.flush_scheduled_tasks().unwrap();
    Ok(SyncReturn(apply_controller_ret(Ok::<_, EaseError>(ret))?))
}

// API_GENERATE_MARKER
pub fn initialize_app(arg: ArgInitializeApp) -> ApiRet {
    let ret = call_controller(controller_initialize_app, arg)?;
    Ok(SyncReturn(ret))
}

pub fn update_storage_permission(arg: bool) -> ApiRet {
    let ret = call_controller(controller_update_storage_permission, arg)?;
    Ok(SyncReturn(ret))
}

pub fn play_music(arg: MusicId) -> ApiRet {
    let ret = call_controller(controller_play_music, arg)?;
    Ok(SyncReturn(ret))
}

pub fn pause_music() -> ApiRet {
    let ret = call_controller(controller_pause_music, ())?;
    Ok(SyncReturn(ret))
}

pub fn resume_music() -> ApiRet {
    let ret = call_controller(controller_resume_music, ())?;
    Ok(SyncReturn(ret))
}

pub fn stop_music() -> ApiRet {
    let ret = call_controller(controller_stop_music, ())?;
    Ok(SyncReturn(ret))
}

pub fn seek_music(arg: ArgSeekMusic) -> ApiRet {
    let ret = call_controller(controller_seek_music, arg)?;
    Ok(SyncReturn(ret))
}

pub fn set_current_music_position_for_player_internal(arg: u64) -> ApiRet {
    let ret = call_controller(
        controller_set_current_music_position_for_player_internal,
        arg,
    )?;
    Ok(SyncReturn(ret))
}

pub fn update_current_music_total_duration_for_player_internal(arg: u64) -> ApiRet {
    let ret = call_controller(
        controller_update_current_music_total_duration_for_player_internal,
        arg,
    )?;
    Ok(SyncReturn(ret))
}

pub fn update_current_music_playing_for_player_internal(arg: bool) -> ApiRet {
    let ret = call_controller(
        controller_update_current_music_playing_for_player_internal,
        arg,
    )?;
    Ok(SyncReturn(ret))
}

pub fn handle_play_music_event_for_player_internal(arg: PlayMusicEventType) -> ApiRet {
    let ret = call_controller(controller_handle_play_music_event_for_player_internal, arg)?;
    Ok(SyncReturn(ret))
}

pub fn play_next_music() -> ApiRet {
    let ret = call_controller(controller_play_next_music, ())?;
    Ok(SyncReturn(ret))
}

pub fn play_previous_music() -> ApiRet {
    let ret = call_controller(controller_play_previous_music, ())?;
    Ok(SyncReturn(ret))
}

pub fn update_music_playmode_to_next() -> ApiRet {
    let ret = call_controller(controller_update_music_playmode_to_next, ())?;
    Ok(SyncReturn(ret))
}

pub fn update_time_to_pause(arg: u64) -> ApiRet {
    let ret = call_controller(controller_update_time_to_pause, arg)?;
    Ok(SyncReturn(ret))
}

pub fn remove_time_to_pause() -> ApiRet {
    let ret = call_controller(controller_remove_time_to_pause, ())?;
    Ok(SyncReturn(ret))
}

pub fn prepare_import_lyric() -> ApiRet {
    let ret = call_controller(controller_prepare_import_lyric, ())?;
    Ok(SyncReturn(ret))
}

pub fn remove_current_music_lyric() -> ApiRet {
    let ret = call_controller(controller_remove_current_music_lyric, ())?;
    Ok(SyncReturn(ret))
}

pub fn change_to_current_music_playlist() -> ApiRet {
    let ret = call_controller(controller_change_to_current_music_playlist, ())?;
    Ok(SyncReturn(ret))
}

pub fn prepare_edit_playlist(arg: PlaylistId) -> ApiRet {
    let ret = call_controller(controller_prepare_edit_playlist, arg)?;
    Ok(SyncReturn(ret))
}

pub fn finish_edit_playlist() -> ApiRet {
    let ret = call_controller(controller_finish_edit_playlist, ())?;
    Ok(SyncReturn(ret))
}

pub fn prepare_edit_playlist_cover() -> ApiRet {
    let ret = call_controller(controller_prepare_edit_playlist_cover, ())?;
    Ok(SyncReturn(ret))
}

pub fn update_edit_playlist_name(arg: String) -> ApiRet {
    let ret = call_controller(controller_update_edit_playlist_name, arg)?;
    Ok(SyncReturn(ret))
}

pub fn clear_edit_playlist_cover() -> ApiRet {
    let ret = call_controller(controller_clear_edit_playlist_cover, ())?;
    Ok(SyncReturn(ret))
}

pub fn prepare_import_entries_in_current_playlist() -> ApiRet {
    let ret = call_controller(controller_prepare_import_entries_in_current_playlist, ())?;
    Ok(SyncReturn(ret))
}

pub fn finish_create_playlist() -> ApiRet {
    let ret = call_controller(controller_finish_create_playlist, ())?;
    Ok(SyncReturn(ret))
}

pub fn clear_create_playlist() -> ApiRet {
    let ret = call_controller(controller_clear_create_playlist, ())?;
    Ok(SyncReturn(ret))
}

pub fn reset_create_playlist_full() -> ApiRet {
    let ret = call_controller(controller_reset_create_playlist_full, ())?;
    Ok(SyncReturn(ret))
}

pub fn prepare_create_playlist_cover() -> ApiRet {
    let ret = call_controller(controller_prepare_create_playlist_cover, ())?;
    Ok(SyncReturn(ret))
}

pub fn prepare_create_playlist_entries() -> ApiRet {
    let ret = call_controller(controller_prepare_create_playlist_entries, ())?;
    Ok(SyncReturn(ret))
}

pub fn prepare_create_playlist() -> ApiRet {
    let ret = call_controller(controller_prepare_create_playlist, ())?;
    Ok(SyncReturn(ret))
}

pub fn update_create_playlist_name(arg: String) -> ApiRet {
    let ret = call_controller(controller_update_create_playlist_name, arg)?;
    Ok(SyncReturn(ret))
}

pub fn clear_create_playlist_cover() -> ApiRet {
    let ret = call_controller(controller_clear_create_playlist_cover, ())?;
    Ok(SyncReturn(ret))
}

pub fn update_create_playlist_mode(arg: CreatePlaylistMode) -> ApiRet {
    let ret = call_controller(controller_update_create_playlist_mode, arg)?;
    Ok(SyncReturn(ret))
}

pub fn change_current_playlist(arg: PlaylistId) -> ApiRet {
    let ret = call_controller(controller_change_current_playlist, arg)?;
    Ok(SyncReturn(ret))
}

pub fn remove_playlist(arg: PlaylistId) -> ApiRet {
    let ret = call_controller(controller_remove_playlist, arg)?;
    Ok(SyncReturn(ret))
}

pub fn remove_music_from_current_playlist(arg: MusicId) -> ApiRet {
    let ret = call_controller(controller_remove_music_from_current_playlist, arg)?;
    Ok(SyncReturn(ret))
}

pub fn play_all_musics() -> ApiRet {
    let ret = call_controller(controller_play_all_musics, ())?;
    Ok(SyncReturn(ret))
}

pub fn clear_edit_playlist_state() -> ApiRet {
    let ret = call_controller(controller_clear_edit_playlist_state, ())?;
    Ok(SyncReturn(ret))
}

pub fn update_root_subkey(arg: RootRouteSubKey) -> ApiRet {
    let ret = call_controller(controller_update_root_subkey, arg)?;
    Ok(SyncReturn(ret))
}

pub fn remove_storage(arg: StorageId) -> ApiRet {
    let ret = call_controller(controller_remove_storage, arg)?;
    Ok(SyncReturn(ret))
}

pub fn locate_entry(arg: String) -> ApiRet {
    let ret = call_controller(controller_locate_entry, arg)?;
    Ok(SyncReturn(ret))
}

pub fn select_entry(arg: String) -> ApiRet {
    let ret = call_controller(controller_select_entry, arg)?;
    Ok(SyncReturn(ret))
}

pub fn toggle_all_checked_entries() -> ApiRet {
    let ret = call_controller(controller_toggle_all_checked_entries, ())?;
    Ok(SyncReturn(ret))
}

pub fn select_storage_in_import(arg: StorageId) -> ApiRet {
    let ret = call_controller(controller_select_storage_in_import, arg)?;
    Ok(SyncReturn(ret))
}

pub fn refresh_current_storage_in_import() -> ApiRet {
    let ret = call_controller(controller_refresh_current_storage_in_import, ())?;
    Ok(SyncReturn(ret))
}

pub fn finish_selected_entries_in_import() -> ApiRet {
    let ret = call_controller(controller_finish_selected_entries_in_import, ())?;
    Ok(SyncReturn(ret))
}

pub fn prepare_edit_storage(arg: Option<StorageId>) -> ApiRet {
    let ret = call_controller(controller_prepare_edit_storage, arg)?;
    Ok(SyncReturn(ret))
}

pub fn upsert_storage(arg: ArgUpsertStorage) -> ApiRet {
    let ret = call_controller(controller_upsert_storage, arg)?;
    Ok(SyncReturn(ret))
}

pub fn test_connection(arg: ArgUpsertStorage) -> ApiRet {
    let ret = call_controller(controller_test_connection, arg)?;
    Ok(SyncReturn(ret))
}
