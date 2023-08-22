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

    pub fn bubble_sort(&self, size: usize) {
        let mut this = self.0.lock();

        let buf = this.fb.buffer_mut();

        buf.chunks_mut(2u32.pow(size as u32) as usize).for_each(|chk| Self::bubble_sort_range(chk));

    }

    fn bubble_sort_range(buf: &mut [u8]) {
        fn fast_int(buf: &[u8]) -> u32 {
            (buf[0] as u32) << 24
            | (buf[1] as u32) << 16
            | (buf[2] as u32) << 8
            | buf[3] as u32
        }

        let mut sorted = false;
        while !sorted {
            sorted = true;

            for mut i in 0..buf.len() / 4 - 1 {
                if fast_int(&buf[i * 4..]) > fast_int(&buf[i * 4 + 4..]) {
                    i *= 4;
                    buf.swap(i + 0, i + 4 + 0);
                    buf.swap(i + 1, i + 4 + 1);
                    buf.swap(i + 2, i + 4 + 2);
                    buf.swap(i + 3, i + 4 + 3);
                    sorted = false;
                }
            }
        }
    }

    pub fn draw_rgb4_block(&self, img: &[u8], width: usize, height: usize) {
        assert_eq!(img.len(), width * height * 4);

        let mut this = self.0.lock();
        let info = this.fb.info();

        if this.pos_y + height > info.height {
            let diff = info.height - (this.pos_y - height);
            let scroll = diff.div_ceil(VERTICAL_STRIDE);

            this.scroll(scroll);
            this.pos_y -= scroll * VERTICAL_STRIDE;
        }

        this.new_line();

        let mapper = this.mapper.clone();

        for y in 0..height {
            for x in 0..width {
                let ip = (y * width + x) * 4;
                let ax =
                    ((this.pos_y + y) * info.stride + this.pos_x + x) * info.bytes_per_pixel;

                mapper.write(&mut this.fb.buffer_mut()[ax..], &[
                    img[ip],
                    img[ip + 1],
                    img[ip + 2],
                ])
            }
        }

        this.pos_y += height.div_ceil(VERTICAL_STRIDE) * VERTICAL_STRIDE;
        if this.pos_y + VERTICAL_STRIDE > info.height {
            this.new_line();
        }
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
                    let ax =
                        ((this.pos_y + y) * info.stride + this.pos_x + x) * info.bytes_per_pixel;

                    let l = cr.raster()[y][x];

                    mapper.write(&mut this.fb.buffer_mut()[ax..], &[l, l, l])
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
        self.pos_x = 0;
        self.pos_y += VERTICAL_STRIDE;

        let mut moves = 0;
        while self.pos_y + VERTICAL_STRIDE > self.fb.info().height {
            self.pos_y -= VERTICAL_STRIDE;
            moves += 1;
        }

        if moves != 0 {
            self.scroll(moves);
        }
    }

    pub fn scroll(&mut self, by: usize) {
        let info = self.fb.info();

        let start = info.stride * by * VERTICAL_STRIDE * info.bytes_per_pixel;
        let blank_from = (info.height - (by * VERTICAL_STRIDE)) * info.stride * info.bytes_per_pixel;

        if by >= blank_from {
            self.clear();
            return;
        }

        self.fb.buffer_mut().copy_within(start.., 0);
        self.fb.buffer_mut()[blank_from..].fill(0x00);
    }
}
