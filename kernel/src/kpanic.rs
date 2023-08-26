use crate::stacktrace::dump_stack;
use crate::{hlt_loop, println, FRAME_BUFFER};
use core::fmt::Display;
use core::fmt::Write;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    kernel_panic(info)
}

pub fn kernel_panic(printable: impl Display) -> ! {
    if let Ok(fb) = FRAME_BUFFER.try_get() {
        // fb.reset();
        // fb.clear_color([0x4c, 0x00, 0x99]);
        // fb.draw_rgb4_block(include_bytes!("res/panic.data"), 128, 128);
    }

    println!("FATAL ERROR:");
    println!("------------------------------------");
    println!("{printable}");
    println!("------------------------------------");

    unsafe {
        dump_stack();
    }

    hlt_loop();
}
