use std::sync::Arc;

use once_cell::sync::Lazy;
use tokio::runtime::Runtime;

use crate::backend_host::BackendHost;

pub static BACKEND: Lazy<Arc<BackendHost>> = Lazy::new(|| BackendHost::new());
pub static CLIENTS: Lazy<AppPods> = Lazy::new(|| AppPods::new());
pub static RT: Lazy<Runtime> = Lazy::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap()
});
