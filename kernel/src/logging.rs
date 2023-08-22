use log::{LevelFilter, Log, Metadata, Record};
use core::fmt::Write;
use crate::kio::KernelIo;

pub struct KernelLogger(());

impl KernelLogger {
    pub fn init() {
        log::set_logger(&Self(())).unwrap();
        log::set_max_level(LevelFilter::Trace);
    }
}

impl Log for KernelLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        writeln!(KernelIo, "[{}] {}", record.level(), record.args()).unwrap();
    }

    fn flush(&self) {}
}
