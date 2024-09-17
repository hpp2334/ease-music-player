use crate::{
    client::{MistyClientHandle, MistyReadonlyClientHandle},
    controllers::MistyControllerContext,
};

pub trait AsMistyClientHandle<'a>: Copy {
    fn handle(self) -> MistyClientHandle<'a>;
}

impl<'a> AsMistyClientHandle<'a> for MistyClientHandle<'a> {
    fn handle(self) -> MistyClientHandle<'a> {
        self
    }
}

impl<'a> AsMistyClientHandle<'a> for &'a MistyControllerContext<'a> {
    fn handle(self) -> MistyClientHandle<'a> {
        self.handle
    }
}

pub trait AsReadonlyMistyClientHandle<'a>: Copy {
    fn readonly_handle(self) -> MistyReadonlyClientHandle<'a>;
}

impl<'a, S> AsReadonlyMistyClientHandle<'a> for S
where
    S: AsMistyClientHandle<'a>,
{
    fn readonly_handle(self) -> MistyReadonlyClientHandle<'a> {
        MistyReadonlyClientHandle {
            inner: self.handle().inner,
        }
    }
}

impl<'a> AsReadonlyMistyClientHandle<'a> for MistyReadonlyClientHandle<'a> {
    fn readonly_handle(self) -> MistyReadonlyClientHandle<'a> {
        self
    }
}
