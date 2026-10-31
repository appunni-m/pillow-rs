use crate::compute::pool_cpu::ops::draw::{BitmapMask, i16_bitmap};
use crate::error::PilError;
use crate::raster::DynamicImage;

pub(crate) fn apply(
    mut image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI16B(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    i16_bitmap::apply(canvas, x, y, mask_width, mask_height, mask, color);
    Ok(image)
}
