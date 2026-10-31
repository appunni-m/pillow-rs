use crate::compute::pool_cpu::ops::draw::{TextComposeResult, text_coords};
use crate::draw::blend_u8;
use crate::raster::DynamicImage;
use image::GenericImageView;

pub(crate) fn apply(
    image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    mask: &[u8],
    fill: (u8, u8, u8, u8),
    color_mask: bool,
) -> Result<TextComposeResult, crate::error::PilError> {
    let DynamicImage::ImageRgb8(mut canvas) = image else {
        return Ok(TextComposeResult::NotApplicable);
    };
    let (image_width, image_height) = canvas.dimensions();
    if image_width == 0 || image_height == 0 {
        return Ok(TextComposeResult::Unchanged);
    }

    let raw = canvas.as_mut();
    for text_y in 0..height {
        for text_x in 0..width {
            let pixel_index = (text_y * width + text_x) as usize;
            let source_offset = if color_mask { pixel_index * 4 } else { pixel_index };
            let coverage = if color_mask {
                mask[source_offset + 3]
            } else {
                mask[source_offset]
            };
            if coverage == 0 {
                continue;
            }
            let Some((image_x, image_y)) = text_coords::destination(
                x,
                y,
                text_x,
                text_y,
                image_width,
                image_height,
            ) else {
                continue;
            };
            let destination_offset =
                (image_y as usize * image_width as usize + image_x as usize) * 3;
            let inverse = 255u16 - u16::from(coverage);
            let source = if color_mask {
                (mask[source_offset], mask[source_offset + 1], mask[source_offset + 2])
            } else {
                (fill.0, fill.1, fill.2)
            };
            raw[destination_offset] =
                blend_u8(source.0, raw[destination_offset], coverage, inverse);
            raw[destination_offset + 1] =
                blend_u8(source.1, raw[destination_offset + 1], coverage, inverse);
            raw[destination_offset + 2] =
                blend_u8(source.2, raw[destination_offset + 2], coverage, inverse);
        }
    }
    Ok(TextComposeResult::Updated(DynamicImage::ImageRgb8(canvas)))
}
