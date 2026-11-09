use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::raster::RgbaImage;

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
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let Some((image_x_start, image_y_start)) =
        region.destination_origin(x, y, image_width, image_height)
    else {
        return;
    };
    for mask_y in region.y_start..region.y_end {
        let image_y = image_y_start + (mask_y - region.y_start);
        for mask_x in region.x_start..region.x_end {
            let coverage = mask.coverage(mask_x, mask_y);
            if mask.should_skip(coverage) {
                continue;
            }
            let image_x = image_x_start + (mask_x - region.x_start);
            let background = canvas.get_pixel(image_x, image_y);
            let color_coverage = if ALPHA_CHANNEL {
                mask.luma_coverage_for_alpha(coverage, background[3])
            } else {
                coverage
            };
            canvas.put_pixel(
                image_x,
                image_y,
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
