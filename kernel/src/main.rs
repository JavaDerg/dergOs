#![no_std]
#![no_main]

extern crate alloc;

mod fault;
mod fb;
mod kio;
mod logging;
mod mem;
mod rng;
mod serial;

use crate::fb::SharedFrameBuffer;
use crate::logging::KernelLogger;
use crate::mem::MemoryManager;
use crate::serial::COM1;
use bootloader_api::config::Mapping;
use bootloader_api::info::Optional;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};
use conquer_once::spin::OnceCell;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::slice_from_raw_parts;
use mem::kalloc::KernelOomHandler;
use spinning_top::RawSpinlock;
use talc::{Talc, Talck};
use x86_64::instructions::hlt;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::page_table::PageTableEntry;
use x86_64::VirtAddr;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ( write!(crate::kio::KernelIo, $($arg)*).unwrap() );
}

#[macro_export]
macro_rules! println {
    () => { writeln!(crate::kio::KernelIo).unwrap() };
    ($($arg:tt)*) => ( writeln!(crate::kio::KernelIo, $($arg)*).unwrap() );
}

#[global_allocator]
static ALLOCATOR: Talck<RawSpinlock, KernelOomHandler> = Talc::new(KernelOomHandler {}).lock();

static FRAME_BUFFER: OnceCell<SharedFrameBuffer> = OnceCell::uninit();

static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config
};

entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(
    BootInfo {
        framebuffer,
        memory_regions,
        physical_memory_offset,
        ..
    }: &'static mut BootInfo,
) -> ! {
    writeln!(&*COM1, "Hello from dergOs!").unwrap();
    KernelLogger::init();

    if let Optional::Some(fb) = framebuffer {
        FRAME_BUFFER.init_once(move || SharedFrameBuffer::new(fb));
    };
    FRAME_BUFFER.try_get().unwrap().clear();

    println!("Starting...");

    for reg in memory_regions.iter() {
        println!("{:?}", reg);
    }

    hlt_loop();

    // SAFETY: We trust that the information provided by BootInfo are correct.
    //         By moving them to the memory manager we prevent further modifications.
    let mem_mng = unsafe {
        MemoryManager::new(
            VirtAddr::new(
                physical_memory_offset
                    .into_option()
                    .expect("physical memory offset must be configured"),
            ),
            memory_regions,
        )
    };

    println!("Hello from dergOs!");
    FRAME_BUFFER.try_get().unwrap().draw_rgb4_block(
        include_bytes!("../../../../../typea java.data"),
        256,
        256,
    );
    println!();

    let (lv4pt, _) = Cr3::read();

    println!("0x{:X}", lv4pt.start_address().as_u64());

    let addr = (*physical_memory_offset.as_ref().unwrap() + lv4pt.start_address().as_u64())
        as *const PageTableEntry;
    let pt = unsafe { &*slice_from_raw_parts(addr, 512) };

    for pte in pt {
        println!("{pte:?}");
    }

    hlt_loop()
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // we don't want to double fault, no unwrap here
    let _ = writeln!(
        &*COM1,
        "\n------------------------------------\nFATAL ERROR\n{info}"
    );

    hlt_loop()
}
