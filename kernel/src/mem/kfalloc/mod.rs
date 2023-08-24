mod list;
mod primitive;

use crate::mem::kfalloc::list::{NodeTraverser, PageNode};
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions, Optional};
use core::ptr::{null, null_mut};
use log::{info, trace, warn};
use x86_64::structures::paging::{FrameAllocator, Page, PageSize, PhysFrame};
use x86_64::VirtAddr;

pub struct KernelFrameAllocator {
    phys_offset: VirtAddr,
    map: &'static MemoryRegions,

    start: *mut PageNode,
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
            .filter(|mr| mr.kind == MemoryRegionKind::Usable);

        let mut last = null_mut::<PageNode>();

        let mut current = MemoryRegion {
            start: 0,
            end: 0,
            kind: MemoryRegionKind::Usable,
        };

        let mut total_c = 0;
        let mut total_s = 0;

        for mr in usable {
            if current.end == mr.start {
                current.end = mr.end;
                continue;
            }

            // SAFETY: the provided memory region are assumed to be unused and correct
            let node = unsafe { self.init_region(&current) };
            if node.is_null() {
                current = *mr;
                continue;
            }

            current = *mr;

            if !last.is_null() {
                // SAFETY: We performed a null check, therefor the pointer is be valid
                //         and has been written too before by `init_region`.
                unsafe {
                    (*last).next = node;
                }
            }
            last = node;

            total_c += 1;
            total_s += (*node).count * 4096;
        }

        // Finalize the current memory region
        let starting_node = unsafe { self.init_region(&current) };
        if !starting_node.is_null() && !last.is_null() {
            // SAFETY: We performed a null check, therefor the pointer is be valid
            //         and has been written too before by `init_region`.
            unsafe {
                (*last).next = starting_node;
            }
        }

        self.start = starting_node;

        // just some stats
        total_c += 1;
        total_s += (*starting_node).count * 4096;

        info!("Found {total_c} allocatable regions");
        info!("      {} kB of usable memory", total_s / 1024);

        for (i, node) in NodeTraverser::new(starting_node.read()).enumerate() {
            info!(
                "{i}. (0x{:X}) {} kB",
                node.this as usize,
                node.count * 4096 / 1024
            );
        }
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

unsafe impl<S: PageSize> FrameAllocator<S> for KernelFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        todo!()
    }
}
