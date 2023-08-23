use x86_64::structures::paging::{Page, PageTable};

pub struct KernelFrameAllocator {
    kernel_page: Page,
}
