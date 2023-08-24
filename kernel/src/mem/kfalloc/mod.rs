mod list;
mod primitive;

use crate::mem::kfalloc::list::{NodeTraverser, PageNode};
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions, Optional};
use core::ops::Not;
use core::ptr::{null, null_mut};
use log::{info, trace, warn};
use x86_64::structures::paging::{FrameAllocator, Page, PageSize, PhysFrame};
use x86_64::VirtAddr;

pub struct KernelFrameAllocator {
    phys_offset: VirtAddr,
    map: &'static MemoryRegions,

    start: *mut PageNode,
}

struct RegionCombiningIter<I> {
    inner: I,
    current: Option<MemoryRegion>,
}

impl KernelFrameAllocator {
    /// Calling this will initialize the provided memory regions.
    /// They may only be initialized **once**.
    ///
    /// # SAFETY:
    /// Must be initialized before usage
    pub unsafe fn new(phys_offset: VirtAddr, map: &'static MemoryRegions) -> Self {
        Self {
            phys_offset,
            map,
            start: null_mut(),
        }
    }

    /// # SAFETY:
    /// - This may only be called ONCE
    /// - The provided memory regions must be unused and correct
    pub unsafe fn init(&mut self) {
        let usable = self
            .map
            .iter()
            .filter(|mr| mr.kind == MemoryRegionKind::Usable)
            .cloned();

        let combiner = RegionCombiningIter {
            inner: usable,
            current: None,
        };

        let mut start = null_mut::<PageNode>();
        let mut last = null_mut::<PageNode>();

        for mr in combiner {
            // SAFETY: the provided memory region are assumed to be unused and correct
            let node = unsafe { self.init_region(&mr) };
            if node.is_null() {
                panic!("Invalid memory region passed to initializer");
            }

            if start.is_null() {
                start = node;
            }

            if !last.is_null() {
                // SAFETY: We performed a null check, therefor the pointer is be valid
                //         and has been written too before by `init_region`.
                unsafe {
                    (*last).next = node;
                }
            }

            last = node;
        }

        // Finalize the current memory region
        self.start = start;

        let mut node = self.start.read();
        info!(
            "(0x{:X}) {} kB - 0x{:X}",
            node.this as usize,
            node.count * 4096 / 1024,
            node.next as usize,
        );
        while let Some(next) = node.next() {
            node = next;
            info!(
                "(0x{:X}) {} kB",
                node.this as usize,
                node.count * 4096 / 1024
            );
        }

        info!("end");
    }

    /// Trims and initializes a memory region and returns a pointer to the respective node.
    ///
    /// **The pointer may be null on failure!**
    ///
    /// # Safety:
    /// The given memory region must be fully available for usage
    unsafe fn init_region(&mut self, reg: &MemoryRegion) -> *mut PageNode {
        assert_eq!(reg.kind, MemoryRegionKind::Usable);

        const PAGE_MASK: u64 = !(4096 - 1);

        let aligned_start = reg.start & PAGE_MASK;
        if aligned_start != reg.start {
            trace!("Miss aligned memory region start: 0x{:X}", reg.start);
        }

        let aligned_end = reg.end & PAGE_MASK;
        if aligned_end != reg.end {
            trace!("Miss aligned memory region end: 0x{:X}", reg.start);
        }

        let pages = (aligned_end - aligned_start) / 4096;
        if pages == 0 {
            warn!("Useless memory region ignored");
            return null_mut();
        }

        let node_ptr = (self.phys_offset + aligned_start).as_mut_ptr::<PageNode>();
        let node = PageNode {
            this: node_ptr,
            next: null_mut(),
            count: pages as usize,
        };

        // SAFETY: we assume that the given memory region is empty and available
        unsafe {
            node_ptr.write(node);
        }

        node_ptr
    }
}

impl<I> Iterator for RegionCombiningIter<I>
where
    I: Iterator<Item = MemoryRegion>,
{
    type Item = MemoryRegion;

    fn next(&mut self) -> Option<Self::Item> {
        const PAGE_MASK: u64 = !(4096 - 1);

        if self.current.is_none() {
            self.current = Some(self.inner.next()?);
        }

        loop {
            let Some(next) = self.inner.next() else {
                return self.current.take();
            };

            let mut current = self.current.take().unwrap();
            if current.end != next.start {
                if (current.end & PAGE_MASK) - (current.start & PAGE_MASK) == 0 {
                    warn!("Useless memory region ignored");
                    return self.next();
                }

                return Some(current);
            }

            current.end = next.end;
            self.current = Some(current);
        }
    }
}

unsafe impl<S: PageSize> FrameAllocator<S> for KernelFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        todo!()
    }
}
