use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};

pub(crate) fn apply(
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageRgba8(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let (image_width, image_height) = canvas.dimensions();
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let Some((image_x_start, image_y_start)) =
        region.destination_origin(x, y, image_width, image_height)
    else {
        return Ok(DynamicImage::ImageRgba8(canvas));
    };
    for mask_y in region.y_start..region.y_end {
        let image_y = image_y_start + (mask_y - region.y_start);
        for mask_x in region.x_start..region.x_end {
            let coverage = mask.coverage(mask_x, mask_y);
            if mask.should_skip(coverage) {
                continue;
            }
            let image_x = image_x_start + (mask_x - region.x_start);
            blend_pixel(&mut canvas, image_x, image_y, coverage, mask, color);
        }
    }
    Ok(DynamicImage::ImageRgba8(canvas))
}

fn blend_pixel(
    canvas: &mut crate::raster::RgbaImage,
    x: u32,
    y: u32,
    coverage: u8,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) {
    let existing = canvas.get_pixel(x, y);
    let color_coverage = mask.luma_coverage_for_alpha(coverage, existing[3]);
    canvas.put_pixel(
        x,
        y,
        Rgba([
            mask.blend_byte(color.0, existing[0], color_coverage),
            mask.blend_byte(color.1, existing[1], color_coverage),
            mask.blend_byte(color.2, existing[2], color_coverage),
            mask.blend_byte(color.3, existing[3], coverage),
        ]),
    );
}
