mod color;

use crate::fb::color::ColorMapper;
use bootloader_api::info::FrameBuffer;
use core::fmt::Write;
use log::error;
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
            error!(
                "width ({}) != stride ({})",
                fb.info().width,
                fb.info().stride
            );

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

    pub fn reset(&self) {
        let mut this = self.0.lock();

        this.pos_x = 0;
        this.pos_y = 0;
    }

    pub fn clear(&self) {
        self.clear_color([0x4c, 0x00, 0x99]);
        // self.0.lock().clear();
    }

    pub fn clear_color(&self, color: [u8; 3]) {
        let mut this = self.0.lock();
        let info = this.fb.info();

        let mapper = this.mapper.clone();

        let mut buf = this.fb.buffer_mut();

        for y in 0..info.height {
            for x in 0..info.width {
                mapper.write(
                    &mut buf[(y * info.width + x) * info.bytes_per_pixel..],
                    &color,
                );
            }
        }
    }

    fn draw_rgb_block(&self, img: &[u8], width: usize, height: usize, stride: usize, center: bool) {
        assert!(stride >= 3);
        assert_eq!(img.len(), width * height * stride);

        let mut this = self.0.lock();
        let info = this.fb.info();

        if center {
            if this.pos_x != 0 {
                this.new_line();
            }

            this.pos_x = info.width / 2 - width / 2;
        }

        if this.pos_y + height > info.height {
            let diff = info.height - (this.pos_y - height);
            let scroll = diff.div_ceil(VERTICAL_STRIDE);

            this.scroll(scroll);
            this.pos_y -= scroll * VERTICAL_STRIDE;
        }

        let mapper = this.mapper.clone();

        for y in 0..height {
            for x in 0..width {
                let ip = (y * width + x) * stride;
                let ax = ((this.pos_y + y) * info.stride + this.pos_x + x) * info.bytes_per_pixel;

                mapper.write(
                    &mut this.fb.buffer_mut()[ax..],
                    &[img[ip], img[ip + 1], img[ip + 2]],
                )
            }
        }

        this.pos_y += height.div_ceil(VERTICAL_STRIDE) * VERTICAL_STRIDE;
        this.new_line();
    }

    pub fn draw_rgb3_block(&self, img: &[u8], width: usize, height: usize, center: bool) {
        self.draw_rgb_block(img, width, height, 3, center);
    }

    pub fn draw_rgb4_block(&self, img: &[u8], width: usize, height: usize, center: bool) {
        self.draw_rgb_block(img, width, height, 4, center);
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
                }
                '\r' => {
                    this.pos_x = 0;
                    continue;
                }
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

                    if l > 0 {}
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
        let blank_from =
            (info.height - (by * VERTICAL_STRIDE)) * info.stride * info.bytes_per_pixel;

        if by >= blank_from {
            self.clear();
            return;
        }

        self.fb.buffer_mut().copy_within(start.., 0);
        self.fb.buffer_mut()[blank_from..].fill(0x00);
    }
}
