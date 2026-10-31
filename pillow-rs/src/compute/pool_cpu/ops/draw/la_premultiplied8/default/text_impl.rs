use crate::compute::pool_cpu::ops::draw::{TextComposeResult, gray_alpha_text};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn apply(
    mut image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) -> Result<TextComposeResult, PilError> {
    let DynamicImage::ImageLaPremultiplied8(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    if !gray_alpha_text::apply(canvas, x, y, width, height, pixels, fill) {
        return Ok(TextComposeResult::Unchanged);
    }
    Ok(TextComposeResult::Updated(image))
}
