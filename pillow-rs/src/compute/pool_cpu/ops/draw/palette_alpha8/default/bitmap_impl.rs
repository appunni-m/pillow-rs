use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::error::PilError;
use crate::raster::{DynamicImage, LumaA, Pixel};

pub(crate) fn apply(
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImagePaletteAlpha8(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let mut hidden_lanes = canvas.take_hidden_pixel_lanes();
    let (image_width, image_height) = canvas.dimensions();
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let Some((image_x_start, image_y_start)) =
        region.destination_origin(x, y, image_width, image_height)
    else {
        super::super::super::helpers::hidden_lanes::restore(&mut canvas, hidden_lanes)?;
        return Ok(DynamicImage::ImagePaletteAlpha8(canvas));
    };
    for mask_y in region.y_start..region.y_end {
        let image_y = image_y_start + (mask_y - region.y_start);
        for mask_x in region.x_start..region.x_end {
            let image_x = image_x_start + (mask_x - region.x_start);
            let coverage = mask.coverage(mask_x, mask_y);
            if mask.should_skip(coverage) {
                continue;
            }
            let background = canvas.get_pixel(image_x, image_y);
            let pixel_index = image_y as usize * image_width as usize + image_x as usize;
            let previous_hidden = hidden_lanes_for_pixel(
                &hidden_lanes,
                pixel_index,
                2,
                [background[0], background[0]],
            )?;
            let index_coverage = mask.luma_coverage_for_alpha(coverage, background[1]);
            let index = mask.blend_byte(color.0, background[0], index_coverage);
            let alpha = mask.blend_byte(color.3, background[1], coverage);
            canvas.put_pixel(image_x, image_y, LumaA([index, alpha]));
            update_hidden_lanes(
                &canvas,
                &mut hidden_lanes,
                pixel_index,
                2,
                [
                    mask.blend_byte(color.1, previous_hidden[0], index_coverage),
                    mask.blend_byte(color.2, previous_hidden[1], index_coverage),
                ],
                [index, index],
                &|pixel| [pixel.channels()[0], pixel.channels()[0]],
            )?;
        }
    }
    super::super::super::helpers::hidden_lanes::restore(&mut canvas, hidden_lanes)?;
    Ok(DynamicImage::ImagePaletteAlpha8(canvas))
}
