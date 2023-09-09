use std::{
    sync::{atomic::AtomicI64, Arc, Mutex},
    time::Duration,
};

use ease_client::modules::timer::to_host::ITimerService;

use crate::fake_player::FakeMusicPlayerRef;

#[derive(Default)]
struct FakeTimerServiceInner {
    current: AtomicI64,
    music_player: Mutex<Option<FakeMusicPlayerRef>>,
}

#[derive(Default, Clone)]
pub struct FakeTimerServiceRef {
    inner: Arc<FakeTimerServiceInner>,
}

impl FakeTimerServiceRef {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn bind_music_player(&mut self, v: FakeMusicPlayerRef) {
        let mut guard = self.inner.music_player.lock().unwrap();
        *guard = Some(v);
    }

    pub fn advance_timer(&self, duration_s: u64) {
        for _ in 0..duration_s {
            self.inner
                .current
                .fetch_add(1000, std::sync::atomic::Ordering::SeqCst);

            let guard = self.inner.music_player.lock().unwrap();
            guard.as_ref().unwrap().advance_1sec();
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

#[async_trait::async_trait]
impl ITimerService for FakeTimerServiceRef {
    fn get_current_time_ms(&self) -> i64 {
        self.inner.current.load(std::sync::atomic::Ordering::SeqCst)
    }
    async fn wait(&self, _duration: Duration) {
        // next tick
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
