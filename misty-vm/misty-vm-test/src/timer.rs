use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

#[derive(Clone)]
pub struct FakeTimers {
    current: Arc<AtomicU64>,
    timers: Arc<RwLock<Vec<(u64, Waker)>>>,
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

        for (_, waker) in to_wake {
            waker.wake();
        }
    }

    pub fn sleep(&self, duration: Duration) -> FakeTimer {
        let wake_time =
            self.current.load(std::sync::atomic::Ordering::Relaxed) + duration.as_millis() as u64;
        FakeTimer {
            wake_time,
            time: self.clone(),
        }
    }

    pub fn get_current_time(&self) -> Duration {
        Duration::from_millis(self.current.load(std::sync::atomic::Ordering::Relaxed))
    }
}

pub struct FakeTimer {
    wake_time: u64,
    time: FakeTimers,
}

impl Future for FakeTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.time.current.load(std::sync::atomic::Ordering::Relaxed) >= self.wake_time {
            Poll::Ready(())
        } else {
            self.time
                .timers
                .write()
                .unwrap()
                .push((self.wake_time, cx.waker().clone()));
            Poll::Pending
        }
    }
}
