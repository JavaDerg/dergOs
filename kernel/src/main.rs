#![feature(abi_x86_interrupt)]
#![no_std]
#![no_main]

extern crate alloc;

mod fault;
mod fb;
mod interrupts;
mod kio;
mod kpanic;
mod logging;
mod mem;
mod rng;
mod serial;
mod stacktrace;

use crate::fb::{Float, SharedFrameBuffer};
use crate::interrupts::init_idt;
use crate::logging::KernelLogger;
use crate::mem::MemoryManager;
use crate::serial::COM1;
use crate::stacktrace::dump_stack;
use bootloader_api::config::Mapping;
use bootloader_api::info::Optional;
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};
use conquer_once::spin::OnceCell;
use core::arch::asm;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::ptr::slice_from_raw_parts;
use log::info;
use mem::kalloc::KernelOomHandler;
use spinning_top::RawSpinlock;
use talc::{Talc, Talck};
use x86_64::instructions::interrupts::int3;
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
    ($($arg:tt)*) => ( core::writeln!(crate::kio::KernelIo, $($arg)*).unwrap() );
}

#[global_allocator]
static ALLOCATOR: Talck<RawSpinlock, KernelOomHandler> = Talc::new(KernelOomHandler {}).lock();

static FRAME_BUFFER: OnceCell<SharedFrameBuffer> = OnceCell::uninit();

static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(Mapping::Dynamic);
    config.kernel_stack_size = 512 * 1024;
    config
};

entry_point!(entry, config = &BOOTLOADER_CONFIG);

static mut STACK_END: u64 = 0;

fn entry(_info: &'static mut BootInfo) -> ! {
    // im just praying that this works
    unsafe {
        asm!(
            "mov QWORD PTR [{stack_end}], rsp",
            "push rax",
            "xor rbp, rbp",
            "call {km}",
            "ud2",
            stack_end = in(reg) &mut STACK_END,
            km = sym kernel_main,
            options(noreturn)
        )
    }
}

fn kernel_main(
    BootInfo {
        framebuffer,
        memory_regions,
        physical_memory_offset,
        ..
    }: &'static mut BootInfo,
) -> ! {
    KernelLogger::init();

    if let Optional::Some(fb) = framebuffer {
        FRAME_BUFFER.init_once(move || SharedFrameBuffer::new(fb));
    };
    FRAME_BUFFER.try_get().unwrap().clear();

    init_idt();

    FRAME_BUFFER.try_get().unwrap().draw_rgb_block(
        include_bytes!("res/3caulk.data"),
        64,
        64,
        4,
        Float::Right,
        false,
    );
    println!();
    FRAME_BUFFER.try_get().unwrap().draw_rgb_block(
        include_bytes!("res/logo2.data"),
        // include_bytes!("res/logo.data"), // stride=3
        256,
        164,
        4,
        Float::Center,
        true,
    );

    println!("Starting dergOs...");

    // SAFETY: We trust that the information provided by BootInfo are correct.
    //         By moving them to the memory manager we prevent further modifications.
    let _mem_mng = unsafe {
        MemoryManager::new(
            VirtAddr::new(
                physical_memory_offset
                    .into_option()
                    .expect("physical memory offset must be configured"),
            ),
            memory_regions,
        )
    };

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
