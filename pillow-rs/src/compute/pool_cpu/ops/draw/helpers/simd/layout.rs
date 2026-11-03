use crate::raster::Mode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DrawLayout {
    pub(super) mode: Mode,
    pub(super) channels: usize,
    pub(super) float_mode: bool,
}

impl DrawLayout {
    pub(super) const fn new(mode: Mode, channels: usize, float_mode: bool) -> Self {
        Self {
            mode,
            channels,
            float_mode,
        }
    }
}

pub(super) fn layout_for_mode(mode: Mode) -> Option<DrawLayout> {
    let (channels, float_mode) = match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => (1, false),
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => (2, false),
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => (3, false),
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32 => (4, false),
        Mode::ImageF32 => (4, true),
        Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => return None,
    };
    Some(DrawLayout::new(mode, channels, float_mode))
}
