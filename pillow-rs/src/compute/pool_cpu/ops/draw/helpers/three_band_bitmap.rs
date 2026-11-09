use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{Rgb, RgbImage};

/// Blend a three-byte raster without adding an intermediate alpha channel.
pub(crate) fn apply<const RGB_PREMULTIPLIED_IS_OPAQUE: bool>(
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
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let Some((image_x_start, image_y_start)) =
        region.destination_origin(x, y, image_width, image_height)
    else {
        return Ok(());
    };
    for mask_y in region.y_start..region.y_end {
        let image_y = image_y_start + (mask_y - region.y_start);
        for mask_x in region.x_start..region.x_end {
            let mask_coverage = mask.coverage(mask_x, mask_y);
            let coverage = if RGB_PREMULTIPLIED_IS_OPAQUE && mask.is_premultiplied() {
                u8::MAX
            } else {
                mask_coverage
            };
            if mask.should_skip(coverage) {
                continue;
            }
            let image_x = image_x_start + (mask_x - region.x_start);
            let x = image_x;
            let y = image_y;
            let pixel_index = y as usize * image_width as usize + x as usize;
            let background = canvas.get_pixel(x, y);
            let previous_hidden =
                hidden_lanes_for_pixel(hidden_lanes, pixel_index, 1, [u8::MAX, 0])?;
            let full_coverage = coverage == 255;
            let pixel = if full_coverage {
                Rgb([color.0, color.1, color.2])
            } else {
                blend_pixel(*background, color, coverage, mask)
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
