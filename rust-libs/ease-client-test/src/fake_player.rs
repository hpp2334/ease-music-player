use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use crate::backend_host::BackendHost;
use ease_client_backend::{IPlayerDelegate, MusicToPlay};
use ease_client_shared::backends::generated::Code;
use ease_client_shared::backends::music::MusicId;
use ease_client_shared::backends::player::{PlayerDelegateEvent, PlayerDurations};
use ease_client_shared::backends::{encode_message_payload, MessagePayload};
use lofty::AudioFile;

struct FakeMusicPlayerInner {
    req_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    last_bytes: Arc<Mutex<Vec<u8>>>,
    playing: Arc<AtomicBool>,
    current_duration: Arc<AtomicU64>,
    total_duration: Arc<AtomicU64>,
    backend_host: Arc<RwLock<Option<Arc<BackendHost>>>>,
}

#[derive(Clone)]
pub struct FakeMusicPlayerRef {
    inner: Arc<FakeMusicPlayerInner>,
}

async fn request_bytes(url: String) -> Vec<u8> {
    let client = reqwest::Client::builder().no_proxy().build().unwrap();
    let resp = client.get(url).send().await.unwrap();
    let bytes = resp.error_for_status().unwrap().bytes().await.unwrap();
    bytes.to_vec()
}

impl FakeMusicPlayerInner {
    fn new() -> Self {
        Self {
            req_handle: Default::default(),
            last_bytes: Default::default(),
            playing: Default::default(),
            current_duration: Default::default(),
            total_duration: Default::default(),
            backend_host: Default::default(),
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

            self.send_player_event(PlayerDelegateEvent::Complete);
        }
    }

    fn send_player_event(&self, evt: PlayerDelegateEvent) {
        let backend_host = self.backend_host.clone();
        tokio::spawn(async move {
            let backend = backend_host.write().unwrap().clone().map(|v| v.backend());
            if let Some(backend) = backend {
                backend
                    .request(MessagePayload {
                        code: Code::OnPlayerEvent,
                        payload: encode_message_payload(evt),
                    })
                    .await
                    .unwrap();
            }
        });
    }
}

impl FakeMusicPlayerRef {
    pub fn new() -> Self {
        FakeMusicPlayerRef {
            inner: Arc::new(FakeMusicPlayerInner::new()),
        }
    }

    pub fn last_bytes(&self) -> Vec<u8> {
        self.inner.last_bytes.lock().unwrap().clone()
    }
    pub fn advance(&self, duration: Duration) {
        self.inner.advance(duration);
    }

    pub fn set_backend(&self, backend_host: Arc<BackendHost>) {
        *self.inner.backend_host.write().unwrap() = Some(backend_host);
    }
}

impl IPlayerDelegate for FakeMusicPlayerRef {
    fn is_playing(&self) -> bool {
        self.inner.playing.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn get_durations(&self) -> PlayerDurations {
        let v = self
            .inner
            .current_duration
            .load(std::sync::atomic::Ordering::SeqCst);

        PlayerDurations {
            current: Duration::from_millis(v),
            buffer: Duration::ZERO,
        }
    }

    fn resume(&self) {
        let prev_playing = self
            .inner
            .playing
            .swap(true, std::sync::atomic::Ordering::SeqCst);
        if prev_playing {
            return;
        }
        self.inner.send_player_event(PlayerDelegateEvent::Play);
    }

    fn pause(&self) {
        self.inner.pause();
        self.inner.send_player_event(PlayerDelegateEvent::Pause);
    }

    fn stop(&self) {
        self.inner.stop();
        self.inner.send_player_event(PlayerDelegateEvent::Stop);
    }

    fn seek(&self, duration: u64) {
        self.inner
            .current_duration
            .store(duration, std::sync::atomic::Ordering::SeqCst);
        self.inner.send_player_event(PlayerDelegateEvent::Seek);
    }

    fn set_music_url(&self, item: MusicToPlay) {
        let inner = self.inner.clone();
        let id = item.id;
        let url = item.url;
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
            let bytes = request_bytes(url).await;
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

            cloned_inner.send_player_event(PlayerDelegateEvent::Total {
                id,
                duration_ms: total_duration,
            });

            let mut last_bytes = cloned_inner.last_bytes.lock().unwrap();
            *last_bytes = bytes.to_vec();
        });
        {
            let mut req_handle = inner.req_handle.lock().unwrap();
            *req_handle = Some(handle);
        }

        self.resume();
    }

    fn request_total_duration(&self, id: MusicId, url: String) {
        let cloned_inner = self.inner.clone();
        tokio::spawn(async move {
            let bytes = request_bytes(url).await;

            let file = lofty::Probe::new(std::io::BufReader::new(std::io::Cursor::new(
                bytes.to_vec(),
            )))
            .guess_file_type()
            .unwrap()
            .read()
            .unwrap();
            let music_properties = file.properties();
            let total_duration = music_properties.duration().as_millis() as u64;
            cloned_inner.send_player_event(PlayerDelegateEvent::Total {
                id,
                duration_ms: total_duration,
            });

            let tag = id3::Tag::read_from2(std::io::Cursor::new(bytes.clone().to_vec())).ok();
            let pic = tag
                .map(|v| v.pictures().next().cloned())
                .unwrap_or_default()
                .map(|pic| pic.data);
            if let Some(pic) = pic {
                cloned_inner.send_player_event(PlayerDelegateEvent::Cover { id, buffer: pic });
            }
        });
    }
}
