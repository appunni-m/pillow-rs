use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::{
    hidden_lanes_for_pixel, update_hidden_lanes,
};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{GrayAlphaImage, Pixel};
use crate::raster::{GenericImage, GenericImageView};

/// Apply Pillow's L-mask semantics to native LA and La sample pairs.
pub(crate) fn apply<const RGBA_MASK: bool>(
    canvas: &mut GrayAlphaImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
    hidden_lanes: &mut Option<HiddenPixelLanes>,
) -> Result<bool, PilError> {
    let (image_width, image_height) = canvas.dimensions();
    if image_width == 0 || image_height == 0 {
        return Ok(false);
    }
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
            blend_pixel::<RGBA_MASK>(
                canvas,
                image_x as u32,
                image_y as u32,
                coverage,
                fill,
                hidden_lanes,
            )?;
        }
    }
    Ok(true)
}

// TODO(pillow-rs#brg0): verify LA/La text alpha-fill behavior in CPU parity.
fn blend_pixel<const RGBA_MASK: bool>(
    canvas: &mut GrayAlphaImage,
    x: u32,
    y: u32,
    coverage: u8,
    fill: (u8, u8, u8, u8),
    hidden_lanes: &mut Option<HiddenPixelLanes>,
) -> Result<(), PilError> {
    let pixel_index = y as usize * canvas.width() as usize + x as usize;
    let background = canvas.get_pixel(x, y);
    let previous_hidden = hidden_lanes_for_pixel(
        hidden_lanes,
        pixel_index,
        2,
        [background[0], background[0]],
    )?;
    // Pillow's fill_mask_L raises the color-channel mask to full coverage only
    // when the destination alpha is zero. Its integer expression is a
    // zero/nonzero alpha test, not proportional alpha scaling.
    let luma_coverage = if !RGBA_MASK && background[1] == 0 {
        u8::MAX
    } else {
        coverage
    };
    let luma = crate::draw::pil_blend(fill.0, background[0], luma_coverage);
    let alpha = crate::draw::pil_blend(fill.3, background[1], coverage);
    canvas.put_pixel(x, y, crate::raster::LumaA([luma, alpha]));
    update_hidden_lanes(
        canvas,
        hidden_lanes,
        pixel_index,
        2,
        [
            crate::draw::pil_blend(fill.1, previous_hidden[0], luma_coverage),
            crate::draw::pil_blend(fill.2, previous_hidden[1], luma_coverage),
        ],
        [luma, luma],
        &|pixel| [pixel.channels()[0], pixel.channels()[0]],
    )
}
