use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{Rgb, RgbImage};
use image::{GenericImage, GenericImageView};

/// Blend a three-byte raster without adding an intermediate alpha channel.
pub(crate) fn apply(
    canvas: &mut RgbImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
    hidden_lanes: &mut Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
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
            let x = image_x as u32;
            let y = image_y as u32;
            let pixel_index = y as usize * image_width as usize + x as usize;
            let background = canvas.get_pixel(x, y);
            let previous_hidden =
                hidden_lanes_for_pixel(hidden_lanes, pixel_index, 1, [u8::MAX, 0])?;
            let full_coverage = coverage == 255;
            let pixel = if full_coverage {
                Rgb([color.0, color.1, color.2])
            } else {
                blend_pixel(background, color, coverage, mask)
            };
            canvas.put_pixel(x, y, pixel);
            update_hidden_lanes(
                canvas,
                hidden_lanes,
                pixel_index,
                1,
                [
                    if full_coverage {
                        color.3
                    } else {
                        mask.blend_byte(color.3, previous_hidden[0], coverage)
                    },
                    0,
                ],
                [u8::MAX, 0],
                &|_| [u8::MAX, 0],
            )?;
        }
    }
    Ok(())
}

#[inline]
fn blend_pixel(
    background: Rgb<u8>,
    color: (u8, u8, u8, u8),
    coverage: u8,
    mask: &BitmapMask<'_>,
) -> Rgb<u8> {
    Rgb([
        mask.blend_byte(color.0, background[0], coverage),
        mask.blend_byte(color.1, background[1], coverage),
        mask.blend_byte(color.2, background[2], coverage),
    ])
}
