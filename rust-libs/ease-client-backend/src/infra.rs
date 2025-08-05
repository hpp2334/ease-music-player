use std::sync::atomic::AtomicBool;

use tracing::subscriber::set_global_default;

fn create_log(dir: &str) -> std::fs::File {
    let p = std::path::Path::new(dir).join("latest.log");
    let _r = std::fs::remove_file(&p);

    std::fs::File::create(&p).unwrap()
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
    let log_file = create_log(dir);
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(trace_level())
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
        .with_max_level(trace_level())
        .with_writer(log_file)
        .with_ansi(false)
        .finish();

    set_global_default(subscriber).unwrap();
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
