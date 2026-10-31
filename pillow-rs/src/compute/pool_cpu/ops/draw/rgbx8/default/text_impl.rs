use crate::compute::pool_cpu::ops::draw::{TextComposeResult, text_coords};
use crate::draw::blend_u8;
use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};
use image::{GenericImage, GenericImageView};

pub(crate) fn apply(
    image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
    color_mask: bool,
) -> Result<TextComposeResult, PilError> {
    let DynamicImage::ImageRgbx8(mut canvas) = image else {
        return Ok(TextComposeResult::NotApplicable);
    };
    let (image_width, image_height) = canvas.dimensions();
    if image_width == 0 || image_height == 0 {
        return Ok(TextComposeResult::Unchanged);
    }

    for text_y in 0..height {
        for text_x in 0..width {
            let offset = ((text_y * width + text_x) * 4) as usize;
            let coverage = pixels[offset + 3];
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
            let destination = canvas.get_pixel(image_x, image_y);
            let inverse = 255u16 - u16::from(coverage);
            let source = if color_mask {
                (pixels[offset], pixels[offset + 1], pixels[offset + 2])
            } else {
                (fill.0, fill.1, fill.2)
            };
            canvas.put_pixel(
                image_x,
                image_y,
                Rgba([
                    blend_u8(source.0, destination[0], coverage, inverse),
                    blend_u8(source.1, destination[1], coverage, inverse),
                    blend_u8(source.2, destination[2], coverage, inverse),
                    u8::MAX,
                ]),
            );
        }
    }
    Ok(TextComposeResult::Updated(DynamicImage::ImageRgbx8(canvas)))
}
