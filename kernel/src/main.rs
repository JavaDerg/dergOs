#![no_std]
#![no_main]

mod fb;
mod logging;
mod rng;
mod serial;

use crate::fb::SharedFrameBuffer;
use bootloader_api::info::Optional;
use bootloader_api::{entry_point, BootInfo};
use core::fmt::Write;
use core::ops::Deref;
use core::panic::PanicInfo;
use crate::logging::KernelLogger;
use crate::serial::COM1;

entry_point!(kernel_main);

fn kernel_main(BootInfo { framebuffer, .. }: &'static mut BootInfo) -> ! {
    writeln!(&*COM1, "Hello from dergOs!").unwrap();
    KernelLogger::init();

    let Optional::Some(fb) = framebuffer else {
        loop {}
    };

    let sfb = SharedFrameBuffer::new(fb);

    sfb.clear();

    writeln!(&sfb, "Booting...").unwrap();

    // do stuff idk

    writeln!(&sfb, "Hello from dergOs!").unwrap();

/*    for i in 0.. {
        write!(&sfb, "{i} ").unwrap();
    }*/

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // we don't want to double fault, no unwrap here
    let _ = writeln!(&*COM1, "\n------------------------------------\nFATAL ERROR\n{info}");

    loop {}
}
