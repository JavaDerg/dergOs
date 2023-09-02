use crate::mem::kfalloc::KernelFrameAllocator;
use bootloader_api::info::MemoryRegions;
use spinning_top::Spinlock;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{FrameAllocator, OffsetPageTable, PageTable, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

pub mod kalloc;
mod kfalloc;

pub struct MemoryManager {
    inner: Spinlock<InnerMemoryManager>,
    phys_offset: VirtAddr,
}

struct InnerMemoryManager {
    regions: &'static MemoryRegions,
    mapper: OffsetPageTable<'static>,
    allocator: &'static KernelFrameAllocator,
}

impl MemoryManager {
    pub unsafe fn new(
        phys_offset: VirtAddr,
        regions: &'static MemoryRegions,
    ) -> &'static MemoryManager {
        let cr3 = Cr3::read().0.start_address().as_u64();
        // SAFETY: the caller of current function has to guarantee phys_offset is correct
        let level4 = unsafe { &mut *((phys_offset.as_u64() + cr3) as *mut PageTable) };

        // SAFETY: the caller of current function has to guarantee phys_offset is correct
        let mapper = unsafe { OffsetPageTable::new(level4, phys_offset) };

        // SAFETY: First time we are touching these regions, therefore we can initialize them
        let mut allocator = unsafe { KernelFrameAllocator::init(phys_offset, regions) };

        let inner = InnerMemoryManager {
            regions,
            mapper,
            allocator,
        };

        let mmf = (phys_offset.as_u64()
            + allocator
                .allocate_frame()
                .expect("we should really have a second one")
                .start_address()
                .as_u64()) as *mut MemoryManager;

        unsafe {
            mmf.write_volatile(MemoryManager {
                inner: Spinlock::new(inner),
                phys_offset,
            })
        };

        unsafe { mmf.as_ref() }.unwrap()
    }

    pub fn translate<T>(&self, addr: PhysAddr) -> *const T {
        translate_(self.phys_offset, addr)
    }

    pub fn translate_mut<T>(&self, addr: PhysAddr) -> *mut T {
        translate_mut_(self.phys_offset, addr)
    }
}

impl InnerMemoryManager {
    pub fn alloc_self(&mut self) {}
}

unsafe impl FrameAllocator<Size4KiB> for InnerMemoryManager {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        todo!()
    }
}

pub fn translate_<T>(offset: VirtAddr, addr: PhysAddr) -> *const T {
    (offset.as_u64() + addr.as_u64()) as *const T
}

pub fn translate_mut_<T>(offset: VirtAddr, addr: PhysAddr) -> *mut T {
    (offset.as_u64() + addr.as_u64()) as *mut T
}
