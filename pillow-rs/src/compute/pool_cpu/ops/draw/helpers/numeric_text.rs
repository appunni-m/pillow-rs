use crate::raster::RgbaImage;
use image::{GenericImage, GenericImageView};

/// Write the fill's raw four-byte sample when a numeric text pixel is covered.
pub(crate) fn apply<const RGBA_MASK: bool>(
    canvas: &mut RgbaImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) {
    let (image_width, image_height) = canvas.dimensions();
    let ink = [fill.0, fill.1, fill.2, fill.3];
    for text_y in 0..height {
        for text_x in 0..width {
            let pixel_index = (text_y * width + text_x) as usize;
            if super::text_mask::coverage::<RGBA_MASK>(pixels, pixel_index) == 0 {
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
            canvas.put_pixel(
                image_x as u32,
                image_y as u32,
                crate::raster::Rgba(ink),
            );
        }
    }
}
