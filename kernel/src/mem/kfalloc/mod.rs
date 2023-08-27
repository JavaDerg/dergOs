mod lla;

use crate::mem::kfalloc::lla::{AlignedNodePage, NodeTraverser, PageNode};
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};

use core::ptr::NonNull;
use log::{info, trace, warn};

use x86_64::structures::paging::{FrameAllocator, Page, PageSize, PhysFrame};
use x86_64::VirtAddr;

pub struct KernelFrameAllocator {
    phys_offset: VirtAddr,
    map: &'static MemoryRegions,
    sync: NonNull<SyncPage>,

    start: NonNull<AlignedNodePage>,
}

struct PageReservingIter<'a> {
    kfa: &'a mut KernelFrameAllocator,
    left: usize,
}

#[repr(align(4096))]
struct SyncPage {}

struct RegionCombiningIter<I> {
    inner: I,
    current: Option<MemoryRegion>,
}

struct PageCommitment {
    start: *const Page,
    count: usize,
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
            sync: NonNull::dangling(),
            start: NonNull::dangling(),
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

        let mut start: Option<NonNull<AlignedNodePage>> = None;
        let mut last: Option<NonNull<AlignedNodePage>> = None;

        for mr in combiner {
            // SAFETY: the provided memory region are assumed to be unused and correct
            let mut node = unsafe { self.init_region(&mr) }
                .expect("Invalid memory region passed to initializer");

            if start.is_none() {
                start = Some(node);
            }

            if let Some(last_n) = &mut last {
                // SAFETY: We performed a null check, therefor the pointer is be valid
                //         and has been written too before by `init_region`.
                unsafe {
                    last_n.as_mut().0.next = Some(node);
                    node.as_mut().0.prev = last;
                }
            }

            last = Some(node);
        }

        // Finalize the current memory region
        self.start = start.expect("no suitable memory regions found");

        // FIXME: start does not get updated properly??
        self.sync = NonNull::new(self.init_sync_page() as *mut _).unwrap();

        for node in NodeTraverser::new(self.start.as_ref().0) {
            info!(
                "(0x{:X}) {} kB - {:?}",
                node.this.as_ptr() as usize,
                node.count * 4096 / 1024,
                node
            );
        }
    }

    /// Trims and initializes a memory region and returns a pointer to the respective node.
    ///
    /// **The pointer may be null on failure!**
    ///
    /// # Safety:
    /// The given memory region must be fully available for usage
    unsafe fn init_region(&mut self, reg: &MemoryRegion) -> Option<NonNull<AlignedNodePage>> {
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
            return None;
        }

        let mut node_ptr =
            NonNull::new((self.phys_offset + aligned_start).as_mut_ptr::<AlignedNodePage>())
                .unwrap();
        let node = PageNode {
            this: node_ptr,
            next: None,
            prev: None,
            count: pages as usize,
        };

        // SAFETY: we assume that the given memory region is empty and available
        unsafe {
            node_ptr.as_ptr().write_volatile(AlignedNodePage(node));
        }

        Some(node_ptr)
    }

    unsafe fn init_sync_page(&mut self) -> *const SyncPage {
        let (sp, _) = self
            .alloc_linear_no_map(1)
            .expect("at least one page should be available");

        let sp = sp.as_mut_ptr::<SyncPage>();
        sp.write_volatile(SyncPage {});
        sp as *const SyncPage
    }

    unsafe fn alloc_linear_no_map(&mut self, cnt: usize) -> Option<(VirtAddr, usize)> {
        let mut node = NodeTraverser::new(self.start.as_ref().0)
            .filter(|n| n.count >= cnt)
            .next()?;

        if node.count == cnt {
            node.prev
                .as_mut()
                .map(|prev| prev.as_mut().0.next = node.next);
            node.next
                .as_mut()
                .map(|next| next.as_mut().0.prev = node.prev);

            return Some((VirtAddr::new(node.this.as_ptr() as u64), cnt));
        }

        let left = node.count - cnt;
        let new = NonNull::new(node.this.as_ptr().add(left)).unwrap();
        // ik we are using this again, but im not sure if llvm can change the location of where we are writing to
        new.as_ptr().write_volatile(AlignedNodePage(PageNode {
            this: new,
            next: node.next,
            prev: node.prev,
            count: left,
        }));

        if let Some(prev) = &mut node.prev {
            prev.as_mut().0.next = Some(new);
        } else {
            self.start = new;
        }
        node.next
            .as_mut()
            .map(|next| next.as_mut().0.prev = Some(new));

        Some((VirtAddr::new(node.this.as_ptr() as u64), cnt))
    }

    unsafe fn reserve_pages(&mut self, cnt: usize) -> PageReservingIter {
        PageReservingIter {
            kfa: self,
            left: cnt,
        }
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

impl<'a> Iterator for PageReservingIter<'a> {
    type Item = ();

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

impl<'a> Drop for PageReservingIter<'a> {
    fn drop(&mut self) {
        todo!()
    }
}

unsafe impl<S: PageSize> FrameAllocator<S> for KernelFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        todo!()
    }
}
