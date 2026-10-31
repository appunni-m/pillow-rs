use crate::compute::pool_cpu::ops::draw::{BitmapMask, three_band_bitmap};
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
    let DynamicImage::ImageHsv8(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    three_band_bitmap::apply(&mut canvas, x, y, mask_width, mask_height, mask, color);
    Ok(DynamicImage::ImageHsv8(canvas))
}
