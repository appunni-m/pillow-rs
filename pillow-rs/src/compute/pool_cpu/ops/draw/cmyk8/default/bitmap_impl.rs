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
    let DynamicImage::ImageCmyk8(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let (image_width, image_height) = canvas.dimensions();
    let ink = [color.0, color.1, color.2, color.3];
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    if let Some((image_x_start, image_y_start)) =
        region.destination_origin(x, y, image_width, image_height)
    {
        for mask_y in region.y_start..region.y_end {
            let image_y = image_y_start + (mask_y - region.y_start);
            for mask_x in region.x_start..region.x_end {
                let coverage = mask.coverage(mask_x, mask_y);
                if mask.should_skip(coverage) {
                    continue;
                }
                let image_x = image_x_start + (mask_x - region.x_start);
                let existing = canvas.get_pixel(image_x, image_y);
                canvas.put_pixel(
                    image_x,
                    image_y,
                    Rgba([
                        mask.blend_byte(ink[0], existing[0], coverage),
                        mask.blend_byte(ink[1], existing[1], coverage),
                        mask.blend_byte(ink[2], existing[2], coverage),
                        mask.blend_byte(ink[3], existing[3], coverage),
                    ]),
                );
            }
        }
    }
    Ok(DynamicImage::ImageCmyk8(canvas))
}

pub(crate) fn apply_luma_mask(
    image: &DynamicImage,
    mask_image: &DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    color: (u8, u8, u8, u8),
) -> Option<DynamicImage> {
    let (DynamicImage::ImageCmyk8(source), DynamicImage::ImageLuma8(mask)) = (image, mask_image)
    else {
        return None;
    };
    let (image_width, image_height) = source.dimensions();
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let mut canvas = source.clone();
    blend_luma_rows(&mut canvas, mask.as_raw(), mask_width, region, x, y, color);
    Some(DynamicImage::ImageCmyk8(canvas))
}

fn blend_luma_rows(
    canvas: &mut crate::raster::RgbaImage,
    mask: &[u8],
    mask_width: u32,
    region: BitmapRegion,
    x: i32,
    y: i32,
    color: (u8, u8, u8, u8),
) {
    let row_stride = canvas.width() as usize * 4;
    let Some((destination_x, destination_y)) =
        region.destination_origin(x, y, canvas.width(), canvas.height())
    else {
        return;
    };
    let destination = canvas.as_mut();
    let ink = [color.0, color.1, color.2, color.3];
    let clipped_width = (region.x_end - region.x_start) as usize;
    let destination_x = destination_x as usize;
    let destination_row_len = clipped_width * 4;
    for mask_y in region.y_start..region.y_end {
        let image_y = (destination_y + (mask_y - region.y_start)) as usize;
        let destination_start = image_y * row_stride + destination_x * 4;
        let destination_row =
            &mut destination[destination_start..destination_start + destination_row_len];
        let mask_row_start = mask_y as usize * mask_width as usize + region.x_start as usize;
        let mask_row_end = mask_y as usize * mask_width as usize + region.x_end as usize;
        let mask_row = &mask[mask_row_start..mask_row_end];
        for (pixel, coverage) in destination_row
            .chunks_exact_mut(4)
            .zip(mask_row.iter().copied())
        {
            if coverage == 0 {
                continue;
            }
            if coverage == 255 {
                pixel.copy_from_slice(&ink);
            } else {
                for channel in 0..4 {
                    pixel[channel] = blend(ink[channel], pixel[channel], coverage);
                }
            }
        }
    }
}

#[inline]
fn blend(ink: u8, background: u8, coverage: u8) -> u8 {
    if coverage == 255 {
        ink
    } else {
        crate::draw::pil_blend(ink, background, coverage)
    }
}
