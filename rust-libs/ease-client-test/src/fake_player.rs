use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use ease_client::{Action, IMusicPlayerService, PlayerEvent, ViewAction};
use hyper::StatusCode;
use lofty::AudioFile;
use misty_vm::AppPod;

#[derive(Clone)]
struct FakeMusicPlayerInner {
    url: Arc<Mutex<String>>,
    req_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    last_bytes: Arc<Mutex<Vec<u8>>>,
    playing: Arc<AtomicBool>,
    current_duration: Arc<AtomicU64>,
    total_duration: Arc<AtomicU64>,
    should_sync_total_duration: Arc<AtomicBool>,
    pending_events: Arc<Mutex<Vec<PlayerEvent>>>,
    pod: AppPod,
}

#[derive(Clone)]
pub struct FakeMusicPlayerRef {
    inner: Arc<FakeMusicPlayerInner>,
}

impl FakeMusicPlayerInner {
    fn new(pod: AppPod) -> Self {
        Self {
            url: Default::default(),
            req_handle: Default::default(),
            last_bytes: Default::default(),
            playing: Default::default(),
            current_duration: Default::default(),
            total_duration: Default::default(),
            should_sync_total_duration: Default::default(),
            pending_events: Default::default(),
            pod,
        }
    }

    fn pause(&self) {
        let prev_playing = self
            .playing
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        if !prev_playing {
            return;
        }
    }

    fn stop(&self) {
        self.pause();
    }

    fn advance(&self, duration: Duration) {
        let playing = self.playing.load(std::sync::atomic::Ordering::SeqCst);
        if !playing {
            return;
        }

        self.current_duration.fetch_add(
            duration.as_millis() as u64,
            std::sync::atomic::Ordering::SeqCst,
        );
        let current_duration = self
            .current_duration
            .load(std::sync::atomic::Ordering::SeqCst);

        let total_duration = self
            .total_duration
            .load(std::sync::atomic::Ordering::SeqCst);
        if total_duration > 0 && current_duration >= total_duration / 1000 * 1000 {
            self.playing
                .store(false, std::sync::atomic::Ordering::SeqCst);
            self.current_duration
                .store(0, std::sync::atomic::Ordering::SeqCst);

            self.push_player_event(PlayerEvent::Complete);
        }
    }

    fn push_player_event(&self, evt: PlayerEvent) {
        let mut w = self.pending_events.lock().unwrap();
        w.push(evt);
    }

    fn flush_player_events(&self) {
        let should_sync = self
            .should_sync_total_duration
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        if should_sync {
            let v = self
                .total_duration
                .load(std::sync::atomic::Ordering::SeqCst);
            self.push_player_event(PlayerEvent::Total { duration_ms: v });
        }

        loop {
            let mut events: Vec<PlayerEvent> = Default::default();

            {
                let mut w = self.pending_events.lock().unwrap();
                std::mem::swap(&mut *w, &mut events);
            }

            if events.is_empty() {
                break;
            }

            for evt in events {
                self.pod.get().emit(Action::View(ViewAction::Player(evt)));
            }
        }
    }
}

impl FakeMusicPlayerRef {
    pub fn new(pod: AppPod) -> Self {
        FakeMusicPlayerRef {
            inner: Arc::new(FakeMusicPlayerInner::new(pod)),
        }
    }

    pub fn last_bytes(&self) -> Vec<u8> {
        self.inner.last_bytes.lock().unwrap().clone()
    }
    pub fn advance(&self, duration: Duration) {
        self.inner.advance(duration);
    }

    pub fn flush_player_events(&self) {
        self.inner.flush_player_events();
    }
}

impl IMusicPlayerService for FakeMusicPlayerRef {
    fn get_current_duration_s(&self) -> u64 {
        let v = self
            .inner
            .current_duration
            .load(std::sync::atomic::Ordering::SeqCst);
        Duration::from_millis(v).as_secs()
    }

    fn resume(&self) {
        let prev_playing = self
            .inner
            .playing
            .swap(true, std::sync::atomic::Ordering::SeqCst);
        if prev_playing {
            return;
        }
        self.inner.push_player_event(PlayerEvent::Play);
    }

    fn pause(&self) {
        self.inner.pause();
        self.inner.push_player_event(PlayerEvent::Pause);
    }

    fn stop(&self) {
        self.inner.stop();
        self.inner.push_player_event(PlayerEvent::Stop);
    }

    fn seek(&self, duration: u64) {
        self.inner
            .current_duration
            .store(duration, std::sync::atomic::Ordering::SeqCst);
    }

    fn set_music_url(&self, url: String) {
        let inner = self.inner.clone();
        {
            let mut url_v = inner.url.lock().unwrap();
            *url_v = url.clone();
        }
        {
            let handle = inner.req_handle.clone();
            let handle = handle.lock().unwrap();
            if let Some(handle) = handle.as_ref() {
                handle.abort();
            }
        }
        self.inner.pause();
        self.inner
            .current_duration
            .store(0, std::sync::atomic::Ordering::SeqCst);
        let cloned_inner = inner.clone();
        let handle = tokio::spawn(async move {
            let resp = reqwest::get(&url).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            let bytes = resp.bytes().await.unwrap();
            let buf_cursor = std::io::Cursor::new(bytes.to_vec());

            let file = lofty::Probe::new(std::io::BufReader::new(buf_cursor))
                .guess_file_type()
                .unwrap()
                .read()
                .unwrap();
            let music_properties = file.properties();
            let total_duration = music_properties.duration().as_millis() as u64;
            cloned_inner
                .total_duration
                .store(total_duration, std::sync::atomic::Ordering::SeqCst);

            cloned_inner
                .should_sync_total_duration
                .store(true, std::sync::atomic::Ordering::SeqCst);

            let mut last_bytes = cloned_inner.last_bytes.lock().unwrap();
            *last_bytes = bytes.to_vec();
        });
        {
            let mut req_handle = inner.req_handle.lock().unwrap();
            *req_handle = Some(handle);
        }

        self.resume();
    }
}
