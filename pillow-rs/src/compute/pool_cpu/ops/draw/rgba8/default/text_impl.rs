use crate::compute::pool_cpu::ops::draw::{TextComposeResult, text_coords};
use crate::draw::blend_u8;
use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};

pub(crate) fn apply<const RGBA_MASK: bool>(
    image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) -> Result<TextComposeResult, PilError> {
    let DynamicImage::ImageRgba8(mut canvas) = image else {
        return Ok(TextComposeResult::NotApplicable);
    };
    let (image_width, image_height) = canvas.dimensions();
    if image_width == 0 || image_height == 0 {
        return Ok(TextComposeResult::Unchanged);
    }

    for text_y in 0..height {
        for text_x in 0..width {
            let pixel_index = (text_y * width + text_x) as usize;
            let coverage = crate::compute::pool_cpu::ops::draw::helpers::text_mask::coverage::<
                RGBA_MASK,
            >(pixels, pixel_index);
            if coverage == 0 {
                continue;
            }
            let Some((image_x, image_y)) =
                text_coords::destination(x, y, text_x, text_y, image_width, image_height)
            else {
                continue;
            };
            let destination = canvas.get_pixel(image_x, image_y);
            let inverse = 255u16 - u16::from(coverage);
            let channel_coverage = if !RGBA_MASK && destination[3] == 0 {
                u8::MAX
            } else {
                coverage
            };
            let channel_inverse = 255u16 - u16::from(channel_coverage);
            let alpha = blend_u8(fill.3, destination[3], coverage, inverse);
            let source = crate::compute::pool_cpu::ops::draw::helpers::text_mask::color::<RGBA_MASK>(
                pixels,
                pixel_index,
                (fill.0, fill.1, fill.2),
            );
            let red = blend_u8(source.0, destination[0], channel_coverage, channel_inverse);
            let green = blend_u8(source.1, destination[1], channel_coverage, channel_inverse);
            let blue = blend_u8(source.2, destination[2], channel_coverage, channel_inverse);
            canvas.put_pixel(image_x, image_y, Rgba([red, green, blue, alpha]));
        }
    }
    Ok(TextComposeResult::Updated(DynamicImage::ImageRgba8(canvas)))
}
