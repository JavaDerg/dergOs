use core::mem::size_of;
use core::ptr::NonNull;
use x86_64::structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB};
use x86_64::VirtAddr;

pub type PhysNode = NonNull<AlignedNodePage>;

#[repr(align(4096))]
pub struct AlignedNodePage(pub PageNode);

static_assertions::const_assert!(size_of::<PageNode>() <= 4096);

pub struct LinkedPages {
    phys_offset: VirtAddr,

    head: Option<PhysNode>,
    tail: Option<PhysNode>,

    len: usize,
}

#[derive(Copy, Clone, Debug)]
pub struct PageNode {
    pub this: PhysNode,
    pub next: Option<PhysNode>,
    pub prev: Option<PhysNode>,
    pub count: usize,
}

pub struct MutCursor<'a> {
    lp: &'a mut LinkedPages,

    cur: Option<PhysNode>,

    index: usize,
}

unsafe impl FrameAllocator<Size4KiB> for LinkedPages {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        todo!()
    }
}

impl FrameDeallocator<Size4KiB> for LinkedPages {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        todo!()
    }
}
