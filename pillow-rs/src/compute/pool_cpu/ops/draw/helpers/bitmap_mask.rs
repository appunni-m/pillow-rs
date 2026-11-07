use crate::raster::Mode;

/// Byte coverage view shared by Draw Bitmap's exact-mode writers.
/// Pillow 12.2 routes `ImageDraw.bitmap` through `ImagingFill2`: mode `1` uses
/// nonzero coverage, `L` blends physical destination bytes and promotes a
/// nonzero mask to full coverage only for zero-alpha destinations, `RGBA`
/// blends from its alpha byte, and `RGBa` applies PREBLEND to each byte.
/// The generated CPU parity matrix exercises each declared mask mode against
/// every supported destination mode, including PA's zero-alpha L promotion.
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
        self.coverage_at(y as usize * self.width as usize + x as usize)
    }

    pub(crate) fn coverage_at(&self, pixel_index: usize) -> u8 {
        match self.mode {
            // Pillow treats any nonzero bilevel mask sample as full coverage.
            Mode::One => u8::from(self.data[pixel_index] > 0) * 255,
            Mode::ImageLuma8 => self.data[pixel_index],
            Mode::ImageRgba8 => self.data[pixel_index * 4 + 3],
            Mode::ImageRgbPremultiplied8 => self.data[pixel_index * 4 + 3],
            _ => unreachable!("bitmap mode was validated before mask iteration"),
        }
    }

    #[inline]
    pub(crate) const fn mode(&self) -> Mode {
        self.mode
    }

    #[inline]
    pub(crate) const fn data(&self) -> &[u8] {
        self.data
    }

    #[inline]
    pub(crate) fn is_premultiplied(&self) -> bool {
        self.mode == Mode::ImageRgbPremultiplied8
    }

    #[inline]
    pub(crate) fn should_skip(&self, coverage: u8) -> bool {
        coverage == 0 && !self.is_premultiplied()
    }

    /// Pillow's Paste.c `fill_mask_L` route promotes non-alpha coverage only
    /// when destination alpha is zero. Binary and RGBA masks keep their own
    /// coverage rules, and RGBa masks use PREBLEND instead.
    #[inline]
    pub(crate) fn luma_coverage_for_alpha(&self, coverage: u8, alpha: u8) -> u8 {
        if self.mode == Mode::ImageLuma8 && coverage != 0 && alpha == 0 {
            u8::MAX
        } else {
            coverage
        }
    }

    #[inline]
    pub(crate) fn blend_byte(&self, ink: u8, background: u8, coverage: u8) -> u8 {
        if self.is_premultiplied() {
            crate::compute::pool_cpu::helpers::bytes::preblend::apply(
                ink,
                background,
                coverage,
            )
        } else {
            crate::draw::pil_blend(ink, background, coverage)
        }
    }
}
