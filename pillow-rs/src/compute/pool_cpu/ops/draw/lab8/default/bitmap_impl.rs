use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::compute::pool_cpu::ops::draw::{BitmapMask, BitmapRegion};
use crate::error::PilError;
use crate::raster::{DynamicImage, Rgb};

// TODO(pillow-rs#3ptb): verify LAB×RGBa bitmap PREBLEND output across varied
// destination bytes, zero mask alpha, and retained hidden fourth lanes.
// VERIFY: source=reviewed@2026-11-26; parity=pass@draw-rerun-3; benchmark=pending
pub(crate) fn apply(
    image: DynamicImage,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLab8(mut canvas) = image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let mut hidden_lanes = canvas.take_hidden_pixel_lanes();
    let (image_width, image_height) = canvas.dimensions();
    let region = BitmapRegion::clipped(x, y, image_width, image_height, mask_width, mask_height);
    let Some((image_x_start, image_y_start)) =
        region.destination_origin(x, y, image_width, image_height)
    else {
        super::super::super::helpers::hidden_lanes::restore(&mut canvas, hidden_lanes)?;
        return Ok(DynamicImage::ImageLab8(canvas));
    };
    for mask_y in region.y_start..region.y_end {
        let image_y = image_y_start + (mask_y - region.y_start);
        for mask_x in region.x_start..region.x_end {
            let coverage = mask.coverage(mask_x, mask_y);
            if mask.should_skip(coverage) {
                continue;
            }
            let image_x = image_x_start + (mask_x - region.x_start);
            let pixel_index = image_y as usize * image_width as usize + image_x as usize;
            let background = canvas.get_pixel(image_x, image_y);
            let previous_hidden =
                hidden_lanes_for_pixel(&hidden_lanes, pixel_index, 1, [u8::MAX, 0])?;
            let pixel = Rgb([
                blend_lab_byte(color.0, background[0], coverage, false, mask),
                blend_lab_byte(color.1, background[1], coverage, true, mask),
                blend_lab_byte(color.2, background[2], coverage, true, mask),
            ]);
            canvas.put_pixel(image_x, image_y, pixel);
            update_hidden_lanes(
                &mut canvas,
                &mut hidden_lanes,
                pixel_index,
                1,
                [mask.blend_byte(color.3, previous_hidden[0], coverage), 0],
                [u8::MAX, 0],
                &|_| [u8::MAX, 0],
            )?;
        }
    }
    super::super::super::helpers::hidden_lanes::restore(&mut canvas, hidden_lanes)?;
    Ok(DynamicImage::ImageLab8(canvas))
}

#[inline]
fn blend_lab_byte(
    ink: u8,
    background: u8,
    coverage: u8,
    chroma: bool,
    mask: &BitmapMask<'_>,
) -> u8 {
    if mask.is_premultiplied() && chroma {
        mask.blend_byte(ink, background.wrapping_sub(128), coverage)
            .wrapping_add(128)
    } else {
        mask.blend_byte(
            if chroma { ink.wrapping_add(128) } else { ink },
            background,
            coverage,
        )
    }
}
