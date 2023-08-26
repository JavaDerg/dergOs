use core::alloc::Layout;
use talc::{OomHandler, Talc};

pub struct KernelOomHandler {}

impl OomHandler for KernelOomHandler {
    fn handle_oom(_talc: &mut Talc<Self>, _layout: Layout) -> Result<(), ()> {
        Err(())
    }
}
