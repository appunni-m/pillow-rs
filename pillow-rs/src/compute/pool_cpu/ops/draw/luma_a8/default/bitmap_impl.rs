use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::error::PilError;
use crate::raster::{DynamicImage, LumaA};
use image::{GenericImage, GenericImageView};

pub(crate) fn apply(
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLumaA8(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let (image_width, image_height) = canvas.dimensions();
    for mask_y in 0..mask_height {
        for mask_x in 0..mask_width {
            let coverage = mask.coverage(mask_x, mask_y);
            if coverage == 0 {
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
            let existing = canvas.get_pixel(image_x as u32, image_y as u32);
            // Pillow writes L directly when destination alpha is zero, then
            // blends both channels by mask coverage in all other cases.
            let luma_coverage = if existing[1] == 0 { 255 } else { coverage };
            let luma = crate::draw::pil_blend(color.0, existing[0], luma_coverage);
            let alpha = crate::draw::pil_blend(color.3, existing[1], coverage);
            canvas.put_pixel(image_x as u32, image_y as u32, LumaA([luma, alpha]));
        }
    }
    Ok(DynamicImage::ImageLumaA8(canvas))
}
