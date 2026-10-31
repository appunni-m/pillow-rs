//! Shared native-byte vector kernels for the Draw architecture strategies.

mod admission;
mod arithmetic;
mod execute;
mod kernels;

pub(super) use admission::{
    has_visible_draw_point, has_visible_draw_rectangle, valid_draw_rectangle,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DrawLayout {
    mode: crate::raster::Mode,
    channels: usize,
    float_mode: bool,
}

impl DrawLayout {
    const fn new(mode: crate::raster::Mode, channels: usize, float_mode: bool) -> Self {
        Self {
            mode,
            channels,
            float_mode,
        }
    }
}

pub(crate) fn is_eligible(
    image: &crate::raster::DynamicImage,
    ops: &[crate::pipeline::PipelineOp],
    mode: crate::raster::Mode,
) -> bool {
    if ops.is_empty() {
        return false;
    }
    let Some(layout) = layout_for_mode(mode) else {
        return false;
    };
    image.mode() == mode && ops.iter().all(|op| admission::supports_for_image(image, op, layout))
}

pub(crate) fn execute<const CHANNELS: usize, const FLOAT_MODE: bool>(
    image: &crate::raster::DynamicImage,
    ops: &[crate::pipeline::PipelineOp],
    mode: crate::raster::Mode,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    let layout = DrawLayout::new(mode, CHANNELS, FLOAT_MODE);
    if image.mode() != mode || layout_for_mode(mode) != Some(layout) {
        return Err(crate::error::PilError::InternalError(
            "Draw architecture strategy does not match the exact mode".into(),
        ));
    }
    execute::execute(image, ops, layout)
}

fn layout_for_mode(mode: crate::raster::Mode) -> Option<DrawLayout> {
    use crate::raster::Mode;

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
