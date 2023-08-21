use core::fmt::{Arguments, Write};
use conquer_once::spin::Lazy;
use spinning_top::Spinlock;
use uart_16550::SerialPort;
use crate::panic;

// referencing https://wiki.osdev.org/Serial_Ports

pub static COM1: Lazy<SharedSerialPort> = Lazy::new(|| SharedSerialPort::init());
const COM1_PORT: u16 = 0x3F8;

pub struct SharedSerialPort(Spinlock<SerialPort>);

impl SharedSerialPort {
    fn init() -> Self {
        let mut port = unsafe { SerialPort::new(COM1_PORT) };
        port.init();

        Self(Spinlock::new(port))
    }
}

impl Write for &SharedSerialPort {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.lock().write_str(s)
    }
}
