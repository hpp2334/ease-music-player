use super::view::RootViewModelState;

#[derive(Debug, uniffi::Record)]
pub struct ResourceToHostAction {
    pub id: u64,
    pub buf: Option<Vec<u8>>,
}

#[derive(Default, uniffi::Record)]
pub struct InvokeRet {
    pub view: Option<RootViewModelState>,
    pub resources: Vec<ResourceToHostAction>,
}
