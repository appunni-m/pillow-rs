use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::raster::{Rgb, RgbImage};
use image::{GenericImage, GenericImageView};

/// Blend a three-byte raster without adding an intermediate alpha channel.
pub(crate) fn apply(
    canvas: &mut RgbImage,
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
            let pixel = if coverage == 255 {
                Rgb([color.0, color.1, color.2])
            } else {
                blend_pixel(canvas.get_pixel(image_x as u32, image_y as u32), color, coverage)
            };
            canvas.put_pixel(image_x as u32, image_y as u32, pixel);
        }
    }
}

#[inline]
fn blend_pixel(background: Rgb<u8>, color: (u8, u8, u8, u8), coverage: u8) -> Rgb<u8> {
    Rgb([
        crate::draw::pil_blend(color.0, background[0], coverage),
        crate::draw::pil_blend(color.1, background[1], coverage),
        crate::draw::pil_blend(color.2, background[2], coverage),
    ])
}
