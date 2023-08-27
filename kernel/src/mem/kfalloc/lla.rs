use bootloader_api::info::Optional;
use core::mem::size_of;
use core::ptr::NonNull;
use log::trace;

#[repr(align(4096))]
#[repr(C)]
pub struct AlignedNodePage(pub PageNode);

static_assertions::const_assert!(size_of::<PageNode>() <= 4096);

#[derive(Copy, Clone, Debug)]
pub struct PageNode {
    pub this: NonNull<AlignedNodePage>,
    pub next: Option<NonNull<AlignedNodePage>>,
    pub prev: Option<NonNull<AlignedNodePage>>,
    pub count: usize,
}

pub struct NodeTraverser {
    start: Option<PageNode>,
}

impl PageNode {
    pub unsafe fn next(&self) -> Option<PageNode> {
        Some(unsafe { self.next?.as_ref() }.0)
    }
}

impl NodeTraverser {
    /// # SAFETY
    /// Provided linked list must be in a valid state
    pub unsafe fn new(start: PageNode) -> Self {
        Self { start: Some(start) }
    }
}

impl Iterator for NodeTraverser {
    type Item = PageNode;

    fn next(&mut self) -> Option<Self::Item> {
        let cur = self.start.take()?;

        self.start = unsafe { cur.next() };

        Some(cur)
    }
}
