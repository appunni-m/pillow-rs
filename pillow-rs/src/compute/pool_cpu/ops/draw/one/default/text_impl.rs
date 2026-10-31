use crate::error::PilError;
use crate::raster::{DynamicImage, Luma};
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
    let DynamicImage::One(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
    let (image_width, image_height) = canvas.dimensions();
    let ink = if fill.0 > 0 { 255 } else { 0 };
    for text_y in 0..height {
        for text_x in 0..width {
            let offset = ((text_y * width + text_x) * 4) as usize;
            if pixels[offset + 3] == 0 {
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
            canvas.put_pixel(image_x as u32, image_y as u32, Luma([ink]));
        }
    }
    Ok(image)
}
