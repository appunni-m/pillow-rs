use crate::raster::RgbImage;
use image::{GenericImage, GenericImageView};

/// Composite RGBA glyph-mask coverage into a native three-byte raster.
pub(crate) fn apply(
    canvas: &mut RgbImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) {
    let (image_width, image_height) = canvas.dimensions();
    for text_y in 0..height {
        for text_x in 0..width {
            let offset = ((text_y * width + text_x) * 4) as usize;
            let coverage = pixels[offset + 3];
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
            let inverse = 255u16 - u16::from(coverage);
            canvas.put_pixel(
                image_x,
                image_y,
                crate::raster::Rgb([
                    crate::draw::blend_u8(fill.0, background[0], coverage, inverse),
                    crate::draw::blend_u8(fill.1, background[1], coverage, inverse),
                    crate::draw::blend_u8(fill.2, background[2], coverage, inverse),
                ]),
            );
        }
    }
}
