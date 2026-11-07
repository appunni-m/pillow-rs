use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::compute::pool_cpu::ops::draw::{TextComposeResult, text_coords};
use crate::draw::blend_u8;
use crate::raster::DynamicImage;

pub(crate) fn apply<const RGBA_MASK: bool>(
    image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    mask: &[u8],
    fill: (u8, u8, u8, u8),
) -> Result<TextComposeResult, crate::error::PilError> {
    let DynamicImage::ImageRgb8(mut canvas) = image else {
        return Ok(TextComposeResult::NotApplicable);
    };
    let (image_width, image_height) = canvas.dimensions();
    if image_width == 0 || image_height == 0 {
        return Ok(TextComposeResult::Unchanged);
    }
    let mut hidden_lanes = canvas.take_hidden_pixel_lanes();
    for text_y in 0..height {
        for text_x in 0..width {
            let pixel_index = (text_y * width + text_x) as usize;
            let coverage = crate::compute::pool_cpu::ops::draw::helpers::text_mask::coverage::<RGBA_MASK>(mask, pixel_index);
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
            let destination_index = image_y as usize * image_width as usize + image_x as usize;
            let previous_hidden = hidden_lanes_for_pixel(
                &hidden_lanes,
                destination_index,
                1,
                [u8::MAX, 0],
            )?;
            let inverse = 255u16 - u16::from(coverage);
            let source = crate::compute::pool_cpu::ops::draw::helpers::text_mask::color::<RGBA_MASK>(
                mask,
                pixel_index,
                (fill.0, fill.1, fill.2),
            );
            {
                let raw = canvas.as_mut();
                raw[destination_offset] =
                    blend_u8(source.0, raw[destination_offset], coverage, inverse);
                raw[destination_offset + 1] =
                    blend_u8(source.1, raw[destination_offset + 1], coverage, inverse);
                raw[destination_offset + 2] =
                    blend_u8(source.2, raw[destination_offset + 2], coverage, inverse);
            }
            update_hidden_lanes(
                &canvas,
                &mut hidden_lanes,
                destination_index,
                1,
                [blend_u8(fill.3, previous_hidden[0], coverage, inverse), 0],
                [u8::MAX, 0],
                &|_| [u8::MAX, 0],
            )?;
        }
    }
    super::super::super::helpers::hidden_lanes::restore(&mut canvas, hidden_lanes)?;
    Ok(TextComposeResult::Updated(DynamicImage::ImageRgb8(canvas)))
}
