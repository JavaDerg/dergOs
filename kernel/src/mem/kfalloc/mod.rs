mod lla;

use crate::mem::kfalloc::lla::{AlignedNodePage, NodeTraverser, PageNode};
use bootloader_api::info::{MemoryRegion, MemoryRegionKind, MemoryRegions};
use core::mem::{forget, ManuallyDrop};
use core::ops::Range;

use core::ptr::NonNull;
use log::{trace, warn};
use spinning_top::lock_api::MutexGuard;
use spinning_top::{RawSpinlock, Spinlock};

use x86_64::structures::paging::{FrameAllocator, Page, PageSize, PhysFrame};
use x86_64::VirtAddr;

static_assertions::const_assert!(core::mem::size_of::<KernelFrameAllocator>() <= 4096);

#[repr(align(4096))]
pub struct KernelFrameAllocator {
    phys_offset: VirtAddr,

    inner: Spinlock<InnerAllocator>,
}

struct InnerAllocator {
    start: Option<NonNull<AlignedNodePage>>,
}

struct PageReservingIter {
    kfa: &'static KernelFrameAllocator,

    left: usize,
}

struct RegionCombiningIter<I> {
    inner: I,
    current: Option<MemoryRegion>,
}

#[must_use]
struct PageRangeLease {
    start: VirtAddr,
    count: usize,

    kfa: &'static KernelFrameAllocator,
}

impl KernelFrameAllocator {
    /// Calling this will initialize the provided memory regions.
    /// They may only be initialized **once**.
    ///
    /// # SAFETY:
    /// - This may only be called ONCE
    /// - The provided memory regions must be unused and correct
    /// - Run before enabling hardware interrupts
    pub unsafe fn init(phys_offset: VirtAddr, map: &'static MemoryRegions) -> &'static Self {
        let mut this = Self {
            phys_offset,
            inner: Spinlock::new(InnerAllocator { start: None }),
        };

        let usable = map
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
            let mut node = unsafe { this.init_region(&mr) }
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
        this.inner.lock().start = Some(start.expect("no suitable memory regions found"));

        this.write_self().as_ref().unwrap()
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

    unsafe fn write_self(mut self) -> *const Self {
        let (sp, _) = self
            .dirty_alloc_linear_no_map(1)
            .expect("at least one page should be available");

        let sp = sp.as_mut_ptr::<Self>();
        sp.write_volatile(self);
        sp as *const Self
    }

    // Should only used for bootstrapping
    unsafe fn dirty_alloc_linear_no_map(&mut self, cnt: usize) -> Option<(VirtAddr, usize)> {
        let mut inner = self.inner.lock();

        let mut node = NodeTraverser::new(inner.start?.as_ref().0)
            .filter(|n| n.count >= cnt)
            .next()?;

        // node is the size we need, we dont make a new one
        if node.count == cnt {
            if let Some(prev) = &mut node.prev {
                prev.as_mut().0.next = node.next;
            } else {
                inner.start = node.next;
            }
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
            inner.start = node.next;
        }
        node.next
            .as_mut()
            .map(|next| next.as_mut().0.prev = Some(new));

        Some((VirtAddr::new(node.this.as_ptr() as u64), cnt))
    }

    unsafe fn reserve_pages(&'static self, cnt: usize) -> PageReservingIter {
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

impl Iterator for PageReservingIter {
    type Item = PageRangeLease;

    fn next(&mut self) -> Option<Self::Item> {
        if self.left == 0 {
            return None;
        }

        let inner = self.kfa.inner.lock();
        // SAFETY: when the page is present it's readable
        let mut node = unsafe { inner.start?.as_ref() }.0;

        // if the node is >= of the node size we remove the node and take it
        Some(if self.left >= node.count {
            self.left -= node.count;

            if node.prev.is_none() {
                inner.start = node.next;
            }

            // SAFETY: we assume the ll is valid
            let _ = node.prev.as_mut().map(|prev| unsafe {
                prev.as_mut().0.next = node.next;
            });
            let _ = node.next.as_mut().map(|next| unsafe {
                next.as_mut().0.prev = node.prev;
            });

            PageRangeLease {
                start: VirtAddr::new(node.this.as_ptr() as u64).unwrap(),
                count: node.count,
                kfa: self.kfa,
            }
        } else {
            node.count -= self.left;
            self.left = 0;

            let origin = VirtAddr::new(node.this.as_ptr() as u64).unwrap();
            // SAFETY: We know we can bump up the pointer as its within the nodes range
            node.this = NonNull::new(unsafe { node.this.as_ptr().add(self.left) }).unwrap();

            if node.prev.is_none() {
                inner.start = origin;
            }

            // SAFETY: we assume the ll is valid
            let _ = node.prev.as_mut().map(|prev| unsafe {
                prev.as_mut().0.next = Some(node.this);
            });
            let _ = node.next.as_mut().map(|next| unsafe {
                next.as_mut().0.prev = Some(node.this);
            });

            // SAFETY:
            unsafe {
                node.this.as_ptr().write_volatile(AlignedNodePage(node));
            }

            PageRangeLease {
                start: origin,
                count: self.left,
                kfa: self.kfa,
            }
        })
    }
}

impl PageRangeLease {
    pub fn release(self) {
        let _ = self;
    }

    pub fn keep(self) {
        forget(self);
    }
}

impl Drop for PageRangeLease {
    fn drop(&mut self) {
        let inner = self.kfa.inner.lock();

        let page = PageNode {
            this: NonNull::new(self.start.as_mut_ptr()).unwrap(),
            next: inner.start,
            prev: None, // this is none on purpose as this will become the first page
            count: self.count,
        };

        if let Some(mut old) = inner.start {
            unsafe { old.as_mut().0.prev = Some(page.this) };
        }

        unsafe {
            page.this.as_ptr().write_volatile(AlignedNodePage(page));
        }
        inner.start = Some(page.this);
    }
}

unsafe impl<S: PageSize> FrameAllocator<S> for KernelFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        todo!()
    }
}
