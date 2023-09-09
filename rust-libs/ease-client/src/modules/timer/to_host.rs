use std::time::Duration;

use misty_vm::misty_service;

#[async_trait::async_trait]
pub trait ITimerService: Send + Sync + 'static {
    fn get_current_time_ms(&self) -> i64;
    async fn wait(&self, duration: Duration);
}
misty_service!(TimerService, ITimerService);

pub struct HostTimerService;

#[async_trait::async_trait]
impl ITimerService for HostTimerService {
    fn get_current_time_ms(&self) -> i64 {
        let now = std::time::SystemTime::now();
        let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
        return duration.as_millis() as i64;
    }
    async fn wait(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}
