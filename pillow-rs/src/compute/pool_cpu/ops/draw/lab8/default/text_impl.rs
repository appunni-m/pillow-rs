use crate::compute::pool_cpu::ops::draw::three_band_text;
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
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLab8(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    three_band_text::apply(canvas, x, y, width, height, pixels, fill);
    Ok(image)
}
