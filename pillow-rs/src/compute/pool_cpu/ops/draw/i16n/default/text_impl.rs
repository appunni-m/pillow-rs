use crate::compute::pool_cpu::ops::draw::helpers::i16_text;
use crate::compute::pool_cpu::ops::draw::TextComposeResult;
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn apply<const RGBA_MASK: bool>(
    image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) -> Result<TextComposeResult, PilError> {
    let DynamicImage::ImageI16N(mut canvas) = image else {
        return Ok(TextComposeResult::NotApplicable);
    };
    if !i16_text::apply::<RGBA_MASK>(&mut canvas, x, y, width, height, pixels, fill) {
        return Ok(TextComposeResult::Unchanged);
    }
    Ok(TextComposeResult::Updated(DynamicImage::ImageI16N(canvas)))
}
