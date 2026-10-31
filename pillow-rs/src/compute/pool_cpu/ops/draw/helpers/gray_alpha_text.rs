use crate::raster::{GrayAlphaImage, Pixel, Rgb};
use image::{GenericImage, GenericImageView};

/// Preserve the shared byte-carrier text blend for the gray-alpha modes.
pub(crate) fn apply(
    canvas: &mut GrayAlphaImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) -> bool {
    let (image_width, image_height) = canvas.dimensions();
    if image_width == 0 || image_height == 0 {
        return false;
    }
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
            blend_pixel(canvas, image_x as u32, image_y as u32, coverage, fill);
        }
    }
    true
}

fn blend_pixel(
    canvas: &mut GrayAlphaImage,
    x: u32,
    y: u32,
    coverage: u8,
    fill: (u8, u8, u8, u8),
) {
    let background = canvas.get_pixel(x, y);
    let inverse = 255u16 - u16::from(coverage);
    let red = crate::draw::blend_u8(fill.0, background[0], coverage, inverse);
    let green = crate::draw::blend_u8(fill.1, background[0], coverage, inverse);
    let blue = crate::draw::blend_u8(fill.2, background[0], coverage, inverse);
    let luma = Rgb([red, green, blue]).to_luma().0[0];
    canvas.put_pixel(x, y, crate::raster::LumaA([luma, u8::MAX]));
}
