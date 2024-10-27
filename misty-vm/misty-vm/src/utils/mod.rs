use std::{cell::Cell, marker::PhantomData, sync::MutexGuard};

pub(crate) unsafe fn extend_lifetime<'a, T>(v: &T) -> &'a T {
    std::mem::transmute(v)
}

pub(crate) type PhantomUnsync = PhantomData<Cell<()>>;
#[allow(dead_code)]
pub(crate) type PhantomUnsend = PhantomData<MutexGuard<'static, ()>>;
