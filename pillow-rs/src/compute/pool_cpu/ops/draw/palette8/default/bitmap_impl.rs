use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::error::PilError;
use crate::raster::{DynamicImage, Luma};

pub(crate) fn apply(
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImagePalette8(mut indices) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let (image_width, image_height) = indices.dimensions();
    let ink = color.0;
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let Some((image_x_start, image_y_start)) =
        region.destination_origin(x, y, image_width, image_height)
    else {
        return Ok(DynamicImage::ImagePalette8(indices));
    };
    for mask_y in region.y_start..region.y_end {
        let image_y = image_y_start + (mask_y - region.y_start);
        for mask_x in region.x_start..region.x_end {
            let coverage = mask.coverage(mask_x, mask_y);
            if mask.should_skip(coverage) {
                continue;
            }
            let image_x = image_x_start + (mask_x - region.x_start);
            let index = indices.get_pixel(image_x, image_y)[0];
            let output = if coverage == 255 {
                ink
            } else {
                mask.blend_byte(ink, index, coverage)
            };
            indices.put_pixel(image_x, image_y, Luma([output]));
        }
    }
    Ok(DynamicImage::ImagePalette8(indices))
}
