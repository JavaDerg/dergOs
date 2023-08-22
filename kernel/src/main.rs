#![no_std]
#![no_main]

mod fb;
mod logging;
mod rng;
mod serial;
mod kio;

use core::arch::asm;
use crate::fb::SharedFrameBuffer;
use bootloader_api::info::{MemoryRegionKind, Optional};
use bootloader_api::{entry_point, BootInfo};
use core::fmt::Write;
use core::hint::black_box;
use core::panic::PanicInfo;
use conquer_once::spin::OnceCell;
use crate::logging::KernelLogger;
use crate::serial::COM1;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ( write!(crate::kio::KernelIo, $($arg)*).unwrap() );
}

#[macro_export]
macro_rules! println {
    () => { writeln!(crate::kio::KernelIo).unwrap() };
    ($($arg:tt)*) => ( writeln!(crate::kio::KernelIo, $($arg)*).unwrap() );
}

static FRAME_BUFFER: OnceCell<SharedFrameBuffer> = OnceCell::uninit();


entry_point!(kernel_main);

fn kernel_main(BootInfo { framebuffer, memory_regions, .. }: &'static mut BootInfo) -> ! {
    writeln!(&*COM1, "Hello from dergOs!").unwrap();
    KernelLogger::init();

    if let Optional::Some(fb) = framebuffer {
        FRAME_BUFFER.init_once(move || SharedFrameBuffer::new(fb));
    };

    println!("Booting...");

    // do stuff idk

    println!("Hello from dergOs!");

    FRAME_BUFFER.try_get().unwrap().draw_rgb4_block(include_bytes!("../../../../../typea java.data"), 256, 256);

    println!();


    let mut total = 0;
    for mr in &**memory_regions {
        // println!("{mr:?}");

        if mr.kind == MemoryRegionKind::Usable {
            total += mr.end - mr.start;
        }
    }

    println!("total usable space: {} kB", total / 1024);

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // we don't want to double fault, no unwrap here
    let _ = writeln!(&*COM1, "\n------------------------------------\nFATAL ERROR\n{info}");

    loop {}
}
