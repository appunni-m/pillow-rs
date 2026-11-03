use crate::error::PilError;
use crate::raster::{DynamicImage, Luma};
use crate::raster::{GenericImage, GenericImageView};

pub(crate) fn apply<const RGBA_MASK: bool>(
    mut image: DynamicImage,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    pixels: &[u8],
    fill: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLuma8(canvas) = &mut image else {
        return Err(PilError::ValueError("mode mismatch".into()));
    };
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
            let x = image_x as u32;
            let y = image_y as u32;
            let background = canvas.get_pixel(x, y)[0];
            canvas.put_pixel(
                x,
                y,
                Luma([crate::draw::pil_blend(fill.0, background, coverage)]),
            );
        }
    }
    Ok(image)
}
