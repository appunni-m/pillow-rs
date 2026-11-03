use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::RgbImage;
use crate::raster::{GenericImage, GenericImageView};

/// Composite RGBA glyph-mask coverage into a native three-byte raster.
pub(crate) fn apply<const RGBA_MASK: bool>(
    canvas: &mut RgbImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
    hidden_lanes: &mut Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
    let (image_width, image_height) = canvas.dimensions();
    for text_y in 0..height {
        for text_x in 0..width {
            let pixel_index = (text_y * width + text_x) as usize;
            let coverage = super::text_mask::coverage::<RGBA_MASK>(pixels, pixel_index);
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
            let destination_index =
                image_y as usize * image_width as usize + image_x as usize;
            let background = canvas.get_pixel(image_x, image_y);
            let previous_hidden =
                hidden_lanes_for_pixel(hidden_lanes, destination_index, 1, [u8::MAX, 0])?;
            let inverse = 255u16 - u16::from(coverage);
            canvas.put_pixel(
                image_x,
                image_y,
                crate::raster::Rgb([
                    crate::draw::blend_u8(fill.0, background[0], coverage, inverse),
                    crate::draw::blend_u8(fill.1, background[1], coverage, inverse),
                    crate::draw::blend_u8(fill.2, background[2], coverage, inverse),
                ]),
            );
            update_hidden_lanes(
                canvas,
                hidden_lanes,
                destination_index,
                1,
                [
                    crate::draw::blend_u8(fill.3, previous_hidden[0], coverage, inverse),
                    0,
                ],
                [u8::MAX, 0],
                &|_| [u8::MAX, 0],
            )?;
        }
    }
    Ok(())
}
