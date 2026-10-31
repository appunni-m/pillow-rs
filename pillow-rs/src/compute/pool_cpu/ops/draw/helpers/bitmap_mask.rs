use crate::raster::Mode;

/// Byte coverage view shared by Draw Bitmap's exact-mode writers.
pub(crate) struct BitmapMask<'a> {
    mode: Mode,
    width: u32,
    data: &'a [u8],
}

impl<'a> BitmapMask<'a> {
    pub(crate) fn new(mode: Mode, width: u32, data: &'a [u8]) -> Self {
        Self { mode, width, data }
    }

    #[inline]
    pub(crate) fn coverage(&self, x: u32, y: u32) -> u8 {
        self.coverage_at((y * self.width + x) as usize)
    }

    pub(crate) fn coverage_at(&self, pixel_index: usize) -> u8 {
        match self.mode {
            // Pillow treats any nonzero bilevel mask sample as full coverage.
            Mode::One => u8::from(self.data[pixel_index] > 0) * 255,
            Mode::ImageLuma8 => self.data[pixel_index],
            Mode::ImageRgba8 => self.data[pixel_index * 4 + 3],
            // ImagingDrawBitmap ignores RGBa's stored alpha and treats each
            // source pixel as fully opaque coverage.
            Mode::ImageRgbPremultiplied8 => 255,
            _ => unreachable!("bitmap mode was validated before mask iteration"),
        }
    }
}
