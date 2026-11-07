use crate::compute::pool_cpu::ops::draw::BitmapMask;
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
            blend_pixel(&mut canvas, image_x as u32, image_y as u32, coverage, mask, color);
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
