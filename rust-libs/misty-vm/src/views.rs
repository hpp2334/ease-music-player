use std::{any::Any, fmt::Debug};

use crate::{
    client::MistyClientInner,
    states::{MistyStateManager, RefMistyStates},
};

pub struct BoxedView {
    inner: Box<dyn Any + Send + Sync>,
}

impl BoxedView {
    pub fn new<R: Any + Default + Send + Sync + 'static>(v: R) -> Self {
        Self { inner: Box::new(v) }
    }

    pub fn cast<R: Any + Default + Send + Sync + 'static>(self) -> R {
        let r: Box<R> = self.inner.downcast().unwrap();
        return *r;
    }
}

pub trait MistyViewModel<R, S> {
    fn update(&self, states: S, s: &mut R);
}

impl<R, S, F> MistyViewModel<R, S> for F
where
    F: Fn(S, &mut R),
{
    fn update(&self, states: S, s: &mut R) {
        self(states, s)
    }
}

pub(crate) trait ErasedMistyViewModel<R> {
    fn should_update(&self, cx: &MistyStateManager) -> bool;
    fn update(&self, cx: &MistyStateManager, s: &mut R);
}

struct BoxedErasedMistyViewModel<R> {
    pub(crate) inner: Box<dyn ErasedMistyViewModel<R> + Send + Sync>,
}
struct BoxedMistyViewModel<R, S> {
    inner: Box<dyn MistyViewModel<R, S> + Send + Sync>,
}

impl<R, S> Debug for BoxedMistyViewModel<R, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxedMistyViewModel").finish()
    }
}

impl<R> Debug for BoxedErasedMistyViewModel<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoxedErasedMistyViewModel").finish()
    }
}

impl<R, S> ErasedMistyViewModel<R> for BoxedMistyViewModel<R, S>
where
    S: RefMistyStates,
{
    fn should_update(&self, cx: &MistyStateManager) -> bool {
        return cx.contains_updated_state(&S::state_ids());
    }
    fn update(&self, cx: &MistyStateManager, s: &mut R) {
        S::extract_refs(cx, |states| {
            self.inner.update(states, s);
        });
    }
}

impl<R, S> BoxedMistyViewModel<R, S> {
    pub fn new<T: MistyViewModel<R, S> + Send + Sync + 'static>(view_model: T) -> Self {
        Self {
            inner: Box::new(view_model),
        }
    }
}

pub struct MistyViewModelManager<R> {
    models: Vec<BoxedErasedMistyViewModel<R>>,
}

#[derive(Default)]
pub struct MistyViewModelManagerBuilder<R> {
    models: Vec<BoxedErasedMistyViewModel<R>>,
}

impl<R> Debug for MistyViewModelManager<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MistyViewModelManager")
            .field("models", &self.models)
            .finish()
    }
}

impl<R> MistyViewModelManagerBuilder<R> {
    pub fn register<'a, S, V>(mut self, view_model: V) -> Self
    where
        S: RefMistyStates + 'static,
        V: MistyViewModel<R, S> + Send + Sync + 'static,
        R: 'static,
    {
        self.models.push(BoxedErasedMistyViewModel {
            inner: Box::new(BoxedMistyViewModel::new(view_model)),
        });
        self
    }

    pub fn build(self) -> MistyViewModelManager<R> {
        MistyViewModelManager {
            models: self.models,
        }
    }
}

pub(crate) trait ViewNotifier: Debug {
    fn build_view(&self, inner: &MistyClientInner) -> BoxedView;
}

impl<R: Default> MistyViewModelManager<R> {
    pub fn builder() -> MistyViewModelManagerBuilder<R> {
        Default::default()
    }
}

impl<R: Any + Default + Send + Sync + 'static> ViewNotifier for MistyViewModelManager<R> {
    fn build_view(&self, inner: &MistyClientInner) -> BoxedView {
        let mut s = R::default();
        for model in self.models.iter() {
            if model.inner.should_update(&inner.state_manager) {
                model.inner.update(&inner.state_manager, &mut s);
            }
        }

        BoxedView::new(s)
    }
}
