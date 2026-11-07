use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::compute::pool_cpu::ops::draw::TextComposeResult;
use crate::error::PilError;
use crate::raster::{DynamicImage, Pixel};

pub(crate) fn apply<const RGBA_MASK: bool>(
    mut image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) -> Result<TextComposeResult, PilError> {
    let DynamicImage::ImagePaletteAlpha8(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let mut hidden_lanes = canvas.take_hidden_pixel_lanes();
    let (image_width, image_height) = canvas.dimensions();
    if image_width == 0 || image_height == 0 {
        return Ok(TextComposeResult::Unchanged);
    }
    for text_y in 0..height {
        for text_x in 0..width {
            let pixel_index = (text_y * width + text_x) as usize;
            let coverage = crate::compute::pool_cpu::ops::draw::helpers::text_mask::coverage::<RGBA_MASK>(pixels, pixel_index);
            if coverage == 0 {
                continue;
            }
            let image_x = i64::from(x) + i64::from(text_x);
            let image_y = i64::from(y) + i64::from(text_y);
            if image_x < 0
                || image_y < 0
                || image_x >= i64::from(image_width)
                || image_y >= i64::from(image_height)
            {
                continue;
            }

            let image_x = image_x as u32;
            let image_y = image_y as u32;
            let background = canvas.get_pixel(image_x, image_y);
            let pixel_index = image_y as usize * image_width as usize + image_x as usize;
            let previous_hidden = hidden_lanes_for_pixel(
                &hidden_lanes,
                pixel_index,
                2,
                [background[0], background[0]],
            )?;
            // Pillow's PA fill_mask_L path treats the palette index as a
            // color channel only when destination alpha is zero.
            let index_coverage = if !RGBA_MASK && background[1] == 0 {
                u8::MAX
            } else {
                coverage
            };
            let index = crate::draw::pil_blend(fill.0, background[0], index_coverage);
            let alpha = crate::draw::pil_blend(fill.3, background[1], coverage);
            canvas.put_pixel(image_x, image_y, crate::raster::LumaA([index, alpha]));
            update_hidden_lanes(
                canvas,
                &mut hidden_lanes,
                pixel_index,
                2,
                [
                    crate::draw::pil_blend(fill.1, previous_hidden[0], index_coverage),
                    crate::draw::pil_blend(fill.2, previous_hidden[1], index_coverage),
                ],
                [index, index],
                &|pixel| [pixel.channels()[0], pixel.channels()[0]],
            )?;
        }
    }
    super::super::super::helpers::hidden_lanes::restore(canvas, hidden_lanes)?;
    Ok(TextComposeResult::Updated(image))
}
