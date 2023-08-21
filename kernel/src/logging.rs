use crate::fb::SharedFrameBuffer;
use log::{LevelFilter, Log, Metadata, Record};
use crate::serial::{COM1, SharedSerialPort};
use core::fmt::Write;
use conquer_once::spin::Lazy;

pub struct KernelLogger {
    _priv: (),
}


impl KernelLogger {
    pub fn init() {
        log::set_logger(&Self {
            _priv: (),
        }).unwrap();
        log::set_max_level(LevelFilter::Trace);
    }
}

impl Log for KernelLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(&*COM1, "[{}] {}", record.level(), record.args()).unwrap();
    }

    fn flush(&self) {}
}
