use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use tracing::subscriber::set_global_default;

pub fn logs_dir(dir: &str) -> PathBuf {
    Path::new(dir).join("logs")
}

fn create_log(dir: &str) -> (PathBuf, std::fs::File) {
    const MAX_LOGS: usize = 30 - 1;

    let dir = logs_dir(dir);
    std::fs::create_dir_all(&dir).unwrap();

    {
        let f = || {
            let mut entries: Vec<_> = std::fs::read_dir(&dir)?.into_iter().collect();
            if entries.len() >= MAX_LOGS {
                entries.sort_by(|a, b| match (a, b) {
                    (Ok(a), Ok(b)) => a.file_name().cmp(&b.file_name()),
                    _ => std::cmp::Ordering::Equal,
                });
                for entry in entries[..entries.len() - MAX_LOGS].iter() {
                    if let Ok(entry) = entry {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }

            Ok::<(), Box<dyn std::error::Error>>(())
        };
        let _ = f();
    }

    let log_name = {
        let now = chrono::Local::now();
        let f = now.format("%Y-%m-%d_%H-%M-%S").to_string();
        format!("{}.txt", f)
    };
    let log_file = dir.join(log_name);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .open(&log_file)
        .unwrap();
    (log_file, file)
}

fn trace_level() -> tracing::Level {
    #[allow(clippy::if_same_then_else)]
    if std::env::var("EBUILD").is_ok() {
        tracing::Level::INFO
    } else {
        tracing::Level::INFO
    }
}

#[cfg(target_os = "android")]
fn setup_subscriber(dir: &str) {
    use tracing_subscriber::layer::SubscriberExt;
    let (p, log_file) = create_log(dir);
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(trace_level())
        .with_writer(log_file)
        .with_ansi(false)
        .finish();
    let subscriber = subscriber.with(tracing_android::layer("com.ease_music_player").unwrap());
    set_global_default(subscriber).unwrap();
    tracing::info!("open log file: {:?}", p);
}

#[cfg(not(target_os = "android"))]
fn setup_subscriber(dir: &str) {
    let (p, log_file) = create_log(dir);
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(trace_level())
        .with_writer(log_file)
        .with_ansi(false)
        .finish();

    set_global_default(subscriber).unwrap();
    tracing::info!("open log file: {:?}", p);
}

fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let stacktrace = std::backtrace::Backtrace::force_capture();

        tracing::error!("panic info: {}", info);
        tracing::error!("panic stacktrace: {}", stacktrace);

        std::process::abort()
    }));
}

pub fn init_infra(dir: &str) {
    static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);
    let is_init = IS_INITIALIZED.swap(true, std::sync::atomic::Ordering::SeqCst);
    std::env::set_var("RUST_BACKTRACE", "1");
    if !is_init {
        setup_subscriber(dir);
        setup_panic_hook();
    }
}
