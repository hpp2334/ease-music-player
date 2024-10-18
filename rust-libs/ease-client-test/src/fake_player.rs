use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use ease_client::{view_models::*, RootViewModelState};
use hyper::StatusCode;
use lofty::AudioFile;
use misty_vm_test::TestAppContainer;

use crate::rt::ASYNC_RT;

#[derive(Clone)]
struct FakeMusicPlayerInner {
    url: Arc<Mutex<String>>,
    req_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    last_bytes: Arc<Mutex<Vec<u8>>>,
    playing: Arc<AtomicBool>,
    current_duration: Arc<AtomicU64>,
    total_duration: Arc<AtomicU64>,
    app_handle: TestAppContainer<RootViewModelState>,
}

#[derive(Clone)]
pub struct FakeMusicPlayerRef {
    inner: Arc<FakeMusicPlayerInner>,
}

impl FakeMusicPlayerInner {
    fn new(app_handle: TestAppContainer<RootViewModelState>) -> Self {
        Self {
            url: Default::default(),
            req_handle: Default::default(),
            last_bytes: Default::default(),
            playing: Default::default(),
            current_duration: Default::default(),
            total_duration: Default::default(),
            app_handle,
        }
    }

    fn pause(&self) {
        let prev_playing = self
            .playing
            .swap(false, std::sync::atomic::Ordering::SeqCst);
        if !prev_playing {
            return;
        }
        self.app_handle.call_controller(
            controller_update_current_music_playing_for_player_internal,
            false,
        );
    }

    fn stop(&self) {
        self.pause();
        self.sync_current_music_position(0);
    }

    fn sync_current_music_position(&self, duration: u64) {
        self.current_duration
            .store(duration, std::sync::atomic::Ordering::SeqCst);
        self.app_handle.call_controller(
            controller_set_current_music_position_for_player_internal,
            duration,
        );
    }

    fn advance_1sec(&self) {
        let playing = self.playing.load(std::sync::atomic::Ordering::SeqCst);
        if !playing {
            return;
        }

        self.current_duration
            .fetch_add(1000, std::sync::atomic::Ordering::SeqCst);
        let current_duration = self
            .current_duration
            .load(std::sync::atomic::Ordering::SeqCst);
        self.sync_current_music_position(current_duration);

        let total_duration = self
            .total_duration
            .load(std::sync::atomic::Ordering::SeqCst);
        if total_duration > 0 && current_duration >= total_duration / 1000 * 1000 {
            self.app_handle.call_controller(
                controller_handle_play_music_event_for_player_internal,
                PlayMusicEventType::Complete,
            );
        }
    }
}

impl FakeMusicPlayerRef {
    pub fn new(app_handle: TestAppContainer<RootViewModelState>) -> Self {
        FakeMusicPlayerRef {
            inner: Arc::new(FakeMusicPlayerInner::new(app_handle)),
        }
    }

    pub fn last_bytes(&self) -> Vec<u8> {
        self.inner.last_bytes.lock().unwrap().clone()
    }
    pub fn advance_1sec(&self) {
        self.inner.advance_1sec();
    }
}

impl IMusicPlayerService for FakeMusicPlayerRef {
    fn resume(&self) {
        let prev_playing = self
            .inner
            .playing
            .swap(true, std::sync::atomic::Ordering::SeqCst);
        if prev_playing {
            return;
        }

        self.inner.app_handle.call_controller(
            controller_update_current_music_playing_for_player_internal,
            true,
        );
    }

    fn pause(&self) {
        self.inner.pause();
    }

    fn stop(&self) {
        self.inner.stop();
    }

    fn seek(&self, duration: u64) {
        self.inner
            .current_duration
            .store(duration, std::sync::atomic::Ordering::SeqCst);
        self.inner.sync_current_music_position(duration);
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
        self.inner.sync_current_music_position(0);
        let cloned_inner = inner.clone();
        let _guard = ASYNC_RT.enter();
        let handle = ASYNC_RT.spawn(async move {
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
            if total_duration > 0 {
                cloned_inner.app_handle.call_controller(
                    controller_update_current_music_total_duration_for_player_internal,
                    total_duration,
                );
            }

            let mut last_bytes = cloned_inner.last_bytes.lock().unwrap();
            *last_bytes = bytes.to_vec();
        });
        {
            let mut req_handle = inner.req_handle.lock().unwrap();
            *req_handle = Some(handle);
        }

        self.inner.sync_current_music_position(0);
        self.resume();
    }
}
