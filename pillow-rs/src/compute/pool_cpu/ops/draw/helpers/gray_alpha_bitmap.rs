use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{GrayAlphaImage, Pixel, Rgb};
use image::{GenericImage, GenericImageView};

/// Write a gray-alpha carrier without changing its exact mode identity.
pub(crate) fn apply(
    canvas: &mut GrayAlphaImage,
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
            blend_pixel(
                canvas,
                image_x as u32,
                image_y as u32,
                coverage,
                mask,
                color,
                hidden_lanes,
            )?;
        }
    }
    Ok(())
}

fn blend_pixel(
    canvas: &mut GrayAlphaImage,
    x: u32,
    y: u32,
    coverage: u8,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
    hidden_lanes: &mut Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
    let pixel_index = y as usize * canvas.width() as usize + x as usize;
    let existing = canvas.get_pixel(x, y);
    let previous_hidden = hidden_lanes_for_pixel(
        hidden_lanes,
        pixel_index,
        2,
        [existing[0], existing[0]],
    )?;
    let luma_coverage = mask.luma_coverage_for_alpha(coverage, existing[1]);
    let red = mask.blend_byte(color.0, existing[0], luma_coverage);
    let green = mask.blend_byte(color.1, existing[0], luma_coverage);
    let blue = mask.blend_byte(color.2, existing[0], luma_coverage);
    let luma = Rgb([red, green, blue]).to_luma().0[0];
    let alpha = mask.blend_byte(color.3, existing[1], coverage);
    canvas.put_pixel(x, y, crate::raster::LumaA([luma, alpha]));
    update_hidden_lanes(
        canvas,
        hidden_lanes,
        pixel_index,
        2,
        [
            mask.blend_byte(color.1, previous_hidden[0], luma_coverage),
            mask.blend_byte(color.2, previous_hidden[1], luma_coverage),
        ],
        [luma, luma],
        &|pixel| [pixel.channels()[0], pixel.channels()[0]],
    )
}
