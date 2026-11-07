use crate::compute::pool_cpu::ops::draw::BitmapMask;
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
    let DynamicImage::One(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let (image_width, image_height) = canvas.dimensions();
    let ink = color.0;
    for mask_y in 0..mask_height {
        for mask_x in 0..mask_width {
            let coverage = mask.coverage(mask_x, mask_y);
            if mask.should_skip(coverage) {
                continue;
            }
            let image_x = x + mask_x as i32;
            let image_y = y + mask_y as i32;
            if image_x < 0
                || image_y < 0
                || image_x as u32 >= image_width
                || image_y as u32 >= image_height
            {
                continue;
            }
            let pixel = if coverage == 255 {
                ink
            } else {
                mask.blend_byte(
                    ink,
                    canvas.get_pixel(image_x as u32, image_y as u32)[0],
                    coverage,
                )
            };
            canvas.put_pixel(image_x as u32, image_y as u32, Luma([pixel]));
        }
    }
    Ok(DynamicImage::One(canvas))
}
