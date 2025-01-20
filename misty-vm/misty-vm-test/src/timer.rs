use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

#[derive(Clone)]
pub struct FakeTimers {
    current: Arc<AtomicU64>,
    timers: Arc<RwLock<Vec<(u64, Box<dyn FnOnce()>)>>>,
}

impl FakeTimers {
    pub fn new() -> Self {
        FakeTimers {
            current: Default::default(),
            timers: Default::default(),
        }
    }

    pub fn advance(&self, duration: Duration) {
        let new_time =
            self.current.load(std::sync::atomic::Ordering::Relaxed) + duration.as_millis() as u64;
        self.current
            .store(new_time, std::sync::atomic::Ordering::Relaxed);

        let mut timers = self.timers.write().unwrap();
        let (to_wake, to_keep): (Vec<_>, Vec<_>) =
            timers.drain(..).partition(|&(t, _)| t <= new_time);

        *timers = to_keep;
        drop(timers);

        for (_, f) in to_wake {
            f();
        }
    }

    pub fn sleep(&self, duration: Duration, f: impl FnOnce() + 'static) {
        let wake_time =
            self.current.load(std::sync::atomic::Ordering::Relaxed) + duration.as_millis() as u64;

        let mut timers = self.timers.write().unwrap();
        timers.push((wake_time, Box::new(f)));
    }

    pub fn get_current_time(&self) -> Duration {
        Duration::from_millis(self.current.load(std::sync::atomic::Ordering::Relaxed))
    }
}
