use crate::error::PilError;
use crate::raster::{DynamicImage, Rgba};
use image::{GenericImage, GenericImageView};

/// TODO(pillow-rs#3ptb): verify integer-packed CMYK Text inks in CPU parity.
/// TODO(pillow-rs#brg0): verify the historical default-fill CMYK Text bytes.
pub(crate) fn apply<const RGBA_MASK: bool>(
    mut image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageCmyk8(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    // Draw resolves integer multi-band inks into their packed channel bytes
    // before this mode-owned raster strategy runs.
    let ink = [fill.0, fill.1, fill.2, fill.3];
    let (image_width, image_height) = canvas.dimensions();
    for text_y in 0..height {
        for text_x in 0..width {
            let pixel_index = (text_y * width + text_x) as usize;
            let coverage = crate::compute::pool_cpu::ops::draw::helpers::text_mask::coverage::<RGBA_MASK>(pixels, pixel_index);
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
            let background = canvas.get_pixel(image_x, image_y);
            let pixel = if coverage == 255 {
                Rgba(ink)
            } else {
                Rgba([
                    crate::draw::pil_blend(ink[0], background[0], coverage),
                    crate::draw::pil_blend(ink[1], background[1], coverage),
                    crate::draw::pil_blend(ink[2], background[2], coverage),
                    crate::draw::pil_blend(ink[3], background[3], coverage),
                ])
            };
            canvas.put_pixel(image_x, image_y, pixel);
        }
    }
    Ok(image)
}
