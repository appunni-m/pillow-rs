use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{GrayAlphaImage, Pixel, Rgb};

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
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let Some((image_x_start, image_y_start)) =
        region.destination_origin(x, y, image_width, image_height)
    else {
        return Ok(());
    };
    for mask_y in region.y_start..region.y_end {
        let image_y = image_y_start + (mask_y - region.y_start);
        for mask_x in region.x_start..region.x_end {
            let coverage = mask.coverage(mask_x, mask_y);
            if mask.should_skip(coverage) {
                continue;
            }
            let image_x = image_x_start + (mask_x - region.x_start);
            blend_pixel(
                canvas,
                image_x,
                image_y,
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
    let previous_hidden =
        hidden_lanes_for_pixel(hidden_lanes, pixel_index, 2, [existing[0], existing[0]])?;
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
