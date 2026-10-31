use crate::compute::pool_cpu::ops::draw::{BitmapMask, raw_four_band_bitmap};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn apply(
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageF32(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    // F stores raw sample bytes in the four-byte carrier; preserve the byte
    // blend instead of interpreting each carrier as an RGBA pixel.
    raw_four_band_bitmap::apply(&mut canvas, x, y, mask_width, mask_height, mask, color);
    Ok(DynamicImage::ImageF32(canvas))
}
