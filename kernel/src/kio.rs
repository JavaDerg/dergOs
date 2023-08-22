use core::fmt::Write;
use crate::FRAME_BUFFER;
use crate::serial::COM1;

pub struct KernelIo;

impl Write for KernelIo {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write!(&*COM1, "{s}")?;

        if let Ok(mut fb) = FRAME_BUFFER.try_get() {
            fb.write_str(s)?;
        }

        Ok(())
    }
}
