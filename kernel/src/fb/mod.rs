mod color;

use crate::fb::color::ColorMapper;
use bootloader_api::info::FrameBuffer;
use core::fmt::Write;
use log::{error, info, trace};
use noto_sans_mono_bitmap::{get_raster, FontWeight, RasterHeight};
use spinning_top::Spinlock;

const FALLBACK_CHAR: char = '?'; // '�'; // doesnt work apparantly :c

const FONT_HEIGHT: usize = 16;
const FONT_RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;

const LINE_SPACING: usize = 2;
const LETTER_SPACING: usize = 0;

const VERTICAL_STRIDE: usize = FONT_HEIGHT + LINE_SPACING;

pub struct SharedFrameBuffer(Spinlock<InnerFrameBuffer>);

struct InnerFrameBuffer {
    fb: &'static mut FrameBuffer,
    mapper: ColorMapper,

    pos_x: usize,
    pos_y: usize,
}

impl SharedFrameBuffer {
    pub fn new(fb: &'static mut FrameBuffer) -> Self {
        if fb.info().width != fb.info().stride {
            // I have to indicate this failure somehow lol
            fb.buffer_mut().fill(0x80);
            error!("width ({}) != stride ({})", fb.info().width, fb.info().stride);

            unimplemented!("this is a message of regret")
        }

        let bpp = fb.info().bytes_per_pixel;

        Self(Spinlock::new(InnerFrameBuffer {
            mapper: ColorMapper::new(fb.info().pixel_format, bpp),
            fb,
            pos_x: 0,
            pos_y: 0,
        }))
    }

    pub fn clear(&self) {
        self.0.lock().clear();
    }
}

impl Write for &SharedFrameBuffer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut this = self.0.lock();
        let info = this.fb.info();

        for c in s.chars() {
            match c {
                '\n' => {
                    this.new_line();
                    continue;
                },
                '\r' => {
                    this.pos_x = 0;
                    continue;
                },
                _ => (),
            }

            let cr = get_raster(c, FontWeight::Regular, FONT_RASTER_HEIGHT).unwrap_or_else(|| {
                get_raster(FALLBACK_CHAR, FontWeight::Regular, FONT_RASTER_HEIGHT)
                    .expect("this should be present")
            });

            if this.pos_x + cr.width() * info.bytes_per_pixel > info.width {
                this.new_line();
            }

            let mapper = this.mapper.clone();

            for y in 0..cr.height() {
                for x in 0..cr.width() {
                    let rx =
                        ((this.pos_y + y) * info.stride + this.pos_x + x) * info.bytes_per_pixel;

                    let l = cr.raster()[y][x];

                    mapper.write(&mut this.fb.buffer_mut()[rx..], &[l, l, l])
                }
            }

            this.pos_x += cr.width() + LETTER_SPACING;
        }

        Ok(())
    }
}

impl InnerFrameBuffer {
    pub fn clear(&mut self) {
        self.fb.buffer_mut().fill(0x00);
    }

    pub fn new_line(&mut self) {
        trace!("new line!");

        self.pos_x = 0;
        self.pos_y += VERTICAL_STRIDE;

        let mut moves = 0;
        while self.pos_y + VERTICAL_STRIDE > self.fb.info().height {
            self.pos_y -= VERTICAL_STRIDE;
            moves += 1;
        }
        trace!("moves: {moves}");
        trace!("final y: {}", self.pos_y);

        if moves != 0 {
            self.scroll(moves);
        }
    }

    pub fn scroll(&mut self, by: usize) {
        let info = self.fb.info();

        let start = info.stride * by;
        let blank_from = info.height / VERTICAL_STRIDE * info.stride;

        if by >= blank_from {
            self.clear();
            return;
        }

        self.fb.buffer_mut().copy_within(start.., 0);
        self.fb.buffer_mut()[blank_from..].fill(0x00);
    }
}
