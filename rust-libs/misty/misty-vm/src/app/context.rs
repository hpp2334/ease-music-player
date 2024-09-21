use std::sync::{Arc, Weak};

use crate::utils::PhantomUnsend;

use super::internal::AppInternal;

#[derive(Clone)]
pub struct AppContext {
    _app: Arc<AppInternal>,
    _unsend: PhantomUnsend,
}

#[derive(Clone)]
pub struct AppAsyncContext {
    _app: Arc<AppInternal>,
}
