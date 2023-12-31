use bootloader_api::info::PixelFormat;
use core::ptr::read_volatile;

type Mapper = fn(&mut [u8], &[u8; 3]);

#[derive(Clone)]
pub struct ColorMapper {
    mapper: Mapper,
}

impl ColorMapper {
    pub fn new(format: PixelFormat, _px_size: usize) -> Self {
        let mapper: Mapper = match format {
            PixelFormat::Rgb => |pos, px| pos[..3].copy_from_slice(&px[..]),
            PixelFormat::Bgr => |pos, px| pos[..3].copy_from_slice(&[px[2], px[1], px[0]]),
            PixelFormat::U8 => write_luminance,
            PixelFormat::Unknown { .. } => unimplemented!(),
            _ => unimplemented!(),
        };

        Self { mapper }
    }

    pub fn write(&self, dst: &mut [u8], rgb: &[u8; 3]) {
        (self.mapper)(dst, rgb);

        // ensure that data will be actually written
        let _ = unsafe { read_volatile(dst.as_ptr()) };
    }
}

fn write_luminance(dst: &mut [u8], &[r, g, b]: &[u8; 3]) {
    dst[0] = (r as f32 * 0.2126) as u8 + (g as f32 * 0.7152) as u8 + (b as f32 * 0.0722) as u8;
}
