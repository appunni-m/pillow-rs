use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
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
    let DynamicImage::ImageLumaA8(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let mut hidden_lanes = canvas.take_hidden_pixel_lanes();
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
            let image_x = image_x as u32;
            let image_y = image_y as u32;
            let pixel_index = image_y as usize * image_width as usize + image_x as usize;
            let existing = canvas.get_pixel(image_x, image_y);
            let previous_hidden = hidden_lanes_for_pixel(
                &hidden_lanes,
                pixel_index,
                2,
                [existing[0], existing[0]],
            )?;
            let luma_coverage = mask.luma_coverage_for_alpha(coverage, existing[1]);
            let luma = mask.blend_byte(color.0, existing[0], luma_coverage);
            let alpha = mask.blend_byte(color.3, existing[1], coverage);
            canvas.put_pixel(image_x, image_y, LumaA([luma, alpha]));
            update_hidden_lanes(
                &canvas,
                &mut hidden_lanes,
                pixel_index,
                2,
                [
                    mask.blend_byte(color.1, previous_hidden[0], luma_coverage),
                    mask.blend_byte(color.2, previous_hidden[1], luma_coverage),
                ],
                [luma, luma],
                &|pixel| [pixel.channels()[0], pixel.channels()[0]],
            )?;
        }
    }
    super::super::super::helpers::hidden_lanes::restore(&mut canvas, hidden_lanes)?;
    Ok(DynamicImage::ImageLumaA8(canvas))
}
