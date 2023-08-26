use crate::hlt_loop;
use crate::stacktrace::dump_stack;
use conquer_once::spin::Lazy;
use log::trace;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.breakpoint.set_handler_fn(breakpoint_handler);
    idt.double_fault.set_handler_fn(double_fault_handler);
    idt
});

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    trace!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);

    unsafe {
        dump_stack();
    }
}

extern "x86-interrupt" fn double_fault_handler(stack_frame: InterruptStackFrame, err: u64) -> ! {
    trace!("DOUBLE FAULT: 0x{:X}\n{:#?}", err, stack_frame);

    unsafe {
        dump_stack();
    }

    hlt_loop()
}
