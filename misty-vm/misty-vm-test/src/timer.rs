use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use tokio::task::LocalSet;

#[derive(Clone)]
pub struct FakeTimers {
    current: Rc<RefCell<u64>>,
    timers: Rc<RefCell<Vec<(u64, Waker)>>>,
}

impl FakeTimers {
    pub fn new() -> Self {
        FakeTimers {
            current: Rc::new(RefCell::new(0)),
            timers: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn advance(&self, duration: Duration) {
        let new_time = *self.current.borrow() + duration.as_millis() as u64;
        *self.current.borrow_mut() = new_time;

        let mut timers = self.timers.borrow_mut();
        let (to_wake, to_keep): (Vec<_>, Vec<_>) =
            timers.drain(..).partition(|&(t, _)| t <= new_time);

        *timers = to_keep;

        for (_, waker) in to_wake {
            waker.wake();
        }
    }

    pub fn sleep(&self, duration: Duration) -> FakeTimer {
        let wake_time = *self.current.borrow() + duration.as_millis() as u64;
        FakeTimer {
            wake_time,
            time: self.clone(),
        }
    }

    pub fn get_current_time(&self) -> Duration {
        Duration::from_millis(*self.current.borrow())
    }
}

pub struct FakeTimer {
    wake_time: u64,
    time: FakeTimers,
}

impl Future for FakeTimer {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if *self.time.current.borrow() >= self.wake_time {
            Poll::Ready(())
        } else {
            self.time
                .timers
                .borrow_mut()
                .push((self.wake_time, cx.waker().clone()));
            Poll::Pending
        }
    }
}
