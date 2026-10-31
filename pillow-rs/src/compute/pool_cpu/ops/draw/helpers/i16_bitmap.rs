use crate::compute::pool_cpu::ops::draw::BitmapMask;
use crate::raster::{ImageBuffer, Luma, Pixel, Rgb};
use image::{GenericImage, GenericImageView};

type Luma16Image = ImageBuffer<Luma<u16>, Vec<u16>>;

/// Preserve the legacy byte-clipped RGB blend and full-frame I;16 rewrite.
pub(crate) fn apply(
    canvas: &mut Luma16Image,
    x: i32,
    y: i32,
    mask_width: u32,
    mask_height: u32,
    mask: &BitmapMask<'_>,
    color: (u8, u8, u8, u8),
) {
    let (image_width, image_height) = canvas.dimensions();
    for image_y in 0..image_height {
        for image_x in 0..image_width {
            let mask_x = i64::from(image_x) - i64::from(x);
            let mask_y = i64::from(image_y) - i64::from(y);
            let coverage = if mask_x >= 0
                && mask_y >= 0
                && mask_x < i64::from(mask_width)
                && mask_y < i64::from(mask_height)
            {
                mask.coverage(mask_x as u32, mask_y as u32)
            } else {
                0
            };
            let background = crate::raster::dynamic::luma16_to_u8(
                canvas.get_pixel(image_x, image_y)[0],
            );
            let rgb = if coverage == 0 {
                Rgb([background, background, background])
            } else {
                Rgb([
                    crate::draw::pil_blend(color.0, background, coverage),
                    crate::draw::pil_blend(color.1, background, coverage),
                    crate::draw::pil_blend(color.2, background, coverage),
                ])
            };
            let luma = rgb.to_luma().0[0];
            canvas.put_pixel(image_x, image_y, Luma([u16::from(luma) * 257]));
        }
    }
}
