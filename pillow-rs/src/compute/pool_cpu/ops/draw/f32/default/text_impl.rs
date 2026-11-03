use crate::compute::pool_cpu::ops::draw::numeric_text;
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
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageF32(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    numeric_text::apply::<RGBA_MASK>(canvas, x, y, width, height, pixels, fill);
    Ok(image)
}
