
use crate::{STACK_END};
use core::arch::asm;

use core::ptr::null;
use log::{info, trace};

// SAFETY: stack frame pointers must be enabled
#[inline(always)]
pub unsafe fn dump_stack() {
    trace!("Stack trace:");
    // error!("lmao did you actually think I would implement this rn? (i tried ;W;)");

    let mut rbp: *const u64;
    asm!("mov {}, rbp", out(reg) rbp);

    loop {
        if rbp == null() {
            info!("rbp is null");
            break;
        } else if rbp as u64 >= STACK_END {
            info!("found end or over stepped");
            break;
        }

        let rip = rbp.add(1).read();
        trace!("    rbp={:?}; rip={:?}", rbp, rip as *const u64);

        rbp = *rbp as *const u64;
    }

    // info!(".debug_info={:?}", get_debug_info());
}
/*
unsafe fn get_debug_info() -> *const () {
    let dbg_info;
    asm!("mov {}, [.debug_info]", out(reg) dbg_info);
    dbg_info
}
*/
