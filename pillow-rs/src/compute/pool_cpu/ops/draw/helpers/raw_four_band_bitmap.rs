use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::raster::RgbaImage;
use crate::raster::{GenericImage, GenericImageView};

/// Blend four stored bytes while leaving the owning mode strategy in control.
pub(crate) fn apply<const ALPHA_CHANNEL: bool>(
    canvas: &mut RgbaImage,
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
            let background = canvas.get_pixel(image_x as u32, image_y as u32);
            let color_coverage = if ALPHA_CHANNEL {
                mask.luma_coverage_for_alpha(coverage, background[3])
            } else {
                coverage
            };
            canvas.put_pixel(
                image_x as u32,
                image_y as u32,
                crate::raster::Rgba([
                    mask.blend_byte(color.0, background[0], color_coverage),
                    mask.blend_byte(color.1, background[1], color_coverage),
                    mask.blend_byte(color.2, background[2], color_coverage),
                    mask.blend_byte(color.3, background[3], coverage),
                ]),
            );
        }
    }
}
