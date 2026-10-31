use crate::error::PilError;
use crate::raster::{DynamicImage, LumaA};
use image::{GenericImage, GenericImageView};

pub(crate) fn apply(
    mut image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLumaA8(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
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
            let x = image_x as u32;
            let y = image_y as u32;
            let background = canvas.get_pixel(x, y);
            let luma_coverage = if background[1] == 0 { 255 } else { coverage };
            let luma = crate::draw::pil_blend(fill.0, background[0], luma_coverage);
            let alpha = crate::draw::pil_blend(fill.3, background[1], coverage);
            canvas.put_pixel(x, y, LumaA([luma, alpha]));
        }
    }
    Ok(image)
}
