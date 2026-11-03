use crate::compute::pool_cpu::ops::draw::{TextComposeResult, gray_alpha_text};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn apply<const RGBA_MASK: bool>(
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
    let mut hidden_lanes = canvas.take_hidden_pixel_lanes();
    let updated = gray_alpha_text::apply::<RGBA_MASK>(
        canvas,
        x,
        y,
        width,
        height,
        pixels,
        fill,
        &mut hidden_lanes,
    )?;
    super::super::super::helpers::hidden_lanes::restore(canvas, hidden_lanes)?;
    if !updated {
        return Ok(TextComposeResult::Unchanged);
    }
    Ok(TextComposeResult::Updated(image))
}
