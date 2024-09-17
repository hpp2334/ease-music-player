use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::Debug,
    marker::PhantomData,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak},
};

use thread_local::ThreadLocal;

use crate::{
    client::{AsMistyClientHandle, AsReadonlyMistyClientHandle, MistyClientInner},
    utils::extend_lifetime,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MistyStateId(TypeId);

impl MistyStateId {
    pub fn new(id: TypeId) -> Self {
        Self(id)
    }
}

pub trait MistyStateTrait: Any + Default + Send + Sync + 'static {
    fn id() -> MistyStateId {
        MistyStateId::new(std::any::TypeId::of::<Self>())
    }

    fn map<'a, R>(cx: impl AsReadonlyMistyClientHandle<'a>, func: impl FnOnce(&Self) -> R) -> R {
        let states = cx.readonly_handle().inner.state_manager.states();
        let binding = states.get::<Self>();
        let state = binding.downcast();
        let ret = func(state.get());
        ret
    }

    fn update<'a, R: 'static>(
        cx: impl AsMistyClientHandle<'a>,
        func: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let client_ref = cx.handle();
        let can_update = client_ref.inner.state_manager.can_update();
        if !can_update {
            let typ_name = std::any::type_name::<Self>();
            let pid = std::thread::current().id();
            panic!("[{:?}] cannot update state {} in this stage", pid, typ_name);
        }

        client_ref.inner.state_manager.add_update_state::<Self>();
        let states = cx.readonly_handle().inner.state_manager.states();
        let binding = states.get::<Self>();
        let ret = {
            let mut state = binding.downcast_mut();
            func(state.get_mut())
        };
        return ret;
    }
}

pub trait RefMistyStates {
    fn state_ids() -> Vec<MistyStateId>;
    fn extract_refs(cx: &MistyStateManager, handler: impl FnOnce(Self))
    where
        Self: Sized;
}

macro_rules! impl_ref_states_tuple {
    ($($n:tt, $t:ident),+) => {
        #[allow(unused_parens)]
        impl<$($t),+> RefMistyStates for ($(&$t),+)
        where
            $($t: MistyStateTrait),+
        {
            fn state_ids() -> Vec<MistyStateId> {
                vec![$($t::id()),+]
            }

            fn extract_refs(cx: &MistyStateManager, handler: impl FnOnce(Self))
            where
                Self: Sized,
            {
                let states = cx.states();
                let t = (
                    $(states.get::<$t>()),+,
                );
                let t = (
                    $(t.$n.downcast()),+,
                );
                let t = (
                    $(
                        unsafe {
                            extend_lifetime(t.$n.get())
                        }
                    ),+
                );

                handler(t)
            }
        }
    };
}

impl_ref_states_tuple!(0, T1);
impl_ref_states_tuple!(0, T1, 1, T2);
impl_ref_states_tuple!(0, T1, 1, T2, 2, T3);
impl_ref_states_tuple!(0, T1, 1, T2, 2, T3, 3, T4);
impl_ref_states_tuple!(0, T1, 1, T2, 2, T3, 3, T4, 4, T5);
impl_ref_states_tuple!(0, T1, 1, T2, 2, T3, 3, T4, 4, T5, 5, T6);
impl_ref_states_tuple!(0, T1, 1, T2, 2, T3, 3, T4, 4, T5, 5, T6, 6, T7);
impl_ref_states_tuple!(0, T1, 1, T2, 2, T3, 3, T4, 4, T5, 5, T6, 6, T7, 7, T8);
impl_ref_states_tuple!(0, T1, 1, T2, 2, T3, 3, T4, 4, T5, 5, T6, 6, T7, 7, T8, 8, T9);
impl_ref_states_tuple!(0, T1, 1, T2, 2, T3, 3, T4, 4, T5, 5, T6, 6, T7, 7, T8, 8, T9, 9, T10);
impl_ref_states_tuple!(
    0, T1, 1, T2, 2, T3, 3, T4, 4, T5, 5, T6, 6, T7, 7, T8, 8, T9, 9, T10, 10, T11
);
impl_ref_states_tuple!(
    0, T1, 1, T2, 2, T3, 3, T4, 4, T5, 5, T6, 6, T7, 7, T8, 8, T9, 9, T10, 10, T11, 11, T12
);

#[derive(Debug, Clone)]
struct BoxedState {
    inner: Arc<RwLock<dyn Any + Send + Sync>>,
}

struct StateRead<'a, T> {
    inner: RwLockReadGuard<'a, dyn Any + Send + Sync>,
    _marker: PhantomData<T>,
}

struct StateWrite<'a, T> {
    inner: RwLockWriteGuard<'a, dyn Any + Send + Sync>,
    _marker: PhantomData<T>,
}

#[derive(Debug)]
pub struct States {
    inner: HashMap<MistyStateId, BoxedState>,
}

#[derive(Debug)]
pub struct MistyStateManager {
    states: States,
    updated_state: ThreadLocal<RefCell<HashSet<MistyStateId>>>,
    depth: ThreadLocal<RefCell<u32>>,
}

impl<'a, T: 'static> StateRead<'a, T> {
    pub fn get(&'a self) -> &'a T {
        self.inner.downcast_ref().unwrap()
    }
}

impl<'a, T: 'static> StateWrite<'a, T> {
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.downcast_mut().unwrap()
    }
}

impl BoxedState {
    pub fn new<T: MistyStateTrait>() -> Self {
        Self {
            inner: Arc::new(RwLock::new(T::default())),
        }
    }

    pub fn downcast<T: MistyStateTrait>(&self) -> StateRead<'_, T> {
        let state = self.inner.read().unwrap();
        StateRead {
            inner: state,
            _marker: Default::default(),
        }
    }

    pub fn downcast_mut<T: MistyStateTrait>(&self) -> StateWrite<'_, T> {
        let state = self.inner.write().unwrap();
        StateWrite {
            inner: state,
            _marker: Default::default(),
        }
    }
}

impl States {
    pub fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub fn register<T: MistyStateTrait>(&mut self) {
        self.inner.insert(T::id(), BoxedState::new::<T>());
    }

    fn get<T: MistyStateTrait>(&self) -> BoxedState {
        let v = self.inner.get(&T::id()).unwrap();
        v.clone()
    }
}

impl MistyStateManager {
    pub fn new(states: States) -> Self {
        MistyStateManager {
            states,
            updated_state: Default::default(),
            depth: Default::default(),
        }
    }

    pub(crate) fn states(&self) -> &States {
        &self.states
    }

    pub(crate) fn enter_mut_span(&self) {
        let mut mut_update = self.depth.get_or_default().borrow_mut();
        *mut_update += 1;
    }
    pub(crate) fn leave_mut_span(&self) -> bool {
        let mut mut_update = self.depth.get_or_default().borrow_mut();
        if *mut_update <= 0 {
            panic!(
                "[Internal Error] MistyClientThisNotifyState depth is {}",
                *mut_update
            );
        }
        *mut_update -= 1;

        let is_depth_zero = *mut_update == 0;
        return is_depth_zero;
    }
    pub(crate) fn clear_updated_states(&self) {
        self.updated_state.get_or_default().borrow_mut().clear();
    }
    pub(crate) fn reset(&self) {
        self.updated_state.get_or_default().borrow_mut().clear();
        *self.depth.get_or_default().borrow_mut() = 0;
    }
    pub(crate) fn can_update(&self) -> bool {
        return *self.depth.get_or_default().borrow() > 0;
    }

    pub(crate) fn add_update_state<S: MistyStateTrait>(&self) {
        self.updated_state
            .get_or_default()
            .borrow_mut()
            .insert(S::id());
    }

    pub(crate) fn contains_updated_state(&self, state_ids: &Vec<MistyStateId>) -> bool {
        let states = self.updated_state.get_or_default().borrow();
        return state_ids
            .into_iter()
            .any(|state_id| states.contains(state_id));
    }
}

pub(crate) struct GuardCleanupStatesForPanic {
    inner: Weak<MistyClientInner>,
    mark: bool,
}
impl GuardCleanupStatesForPanic {
    pub fn new(inner: Weak<MistyClientInner>) -> GuardCleanupStatesForPanic {
        GuardCleanupStatesForPanic { inner, mark: false }
    }
    pub fn mark(&mut self) {
        self.mark = true;
    }
}
impl Drop for GuardCleanupStatesForPanic {
    fn drop(&mut self) {
        if self.mark {
            return;
        }
        if let Some(inner) = self.inner.upgrade() {
            inner.state_manager.reset();
        }
    }
}
