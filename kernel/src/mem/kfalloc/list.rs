#[repr(align(4096))]
#[repr(C)]
pub struct PageNode {
    pub this: *mut PageNode,
    pub next: *mut PageNode,
    pub count: usize,
}

pub struct NodeTraverser {
    start: Option<PageNode>,
}

impl PageNode {
    pub unsafe fn next(&self) -> Option<PageNode> {
        if self.next.is_null() {
            return None;
        }

        Some(unsafe { self.next.read() })
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

        if !cur.next.is_null() {
            self.start = Some(unsafe { cur.next.read() });
        }

        Some(cur)
    }
}
