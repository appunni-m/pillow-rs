use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::raster::{GrayAlphaImage, Pixel, Rgb};
use image::{GenericImage, GenericImageView};

/// Write a gray-alpha carrier without changing its exact mode identity.
pub(crate) fn apply(
    canvas: &mut GrayAlphaImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) {
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
            blend_pixel(canvas, image_x as u32, image_y as u32, coverage, color);
        }
    }
}

fn blend_pixel(
    canvas: &mut GrayAlphaImage,
    x: u32,
    y: u32,
    coverage: u8,
    color: (u8, u8, u8, u8),
) {
    let existing = canvas.get_pixel(x, y);
    let red = crate::draw::pil_blend(color.0, existing[0], coverage);
    let green = crate::draw::pil_blend(color.1, existing[0], coverage);
    let blue = crate::draw::pil_blend(color.2, existing[0], coverage);
    let luma = Rgb([red, green, blue]).to_luma().0[0];
    let alpha = crate::draw::pil_blend(color.3, existing[1], coverage);
    canvas.put_pixel(x, y, crate::raster::LumaA([luma, alpha]));
}
