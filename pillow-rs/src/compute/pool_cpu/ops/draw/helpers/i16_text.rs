use crate::draw::blend_u8;
use crate::raster::{ImageBuffer, Luma, Pixel, Rgb};
use image::{GenericImage, GenericImageView};

type I16Image = ImageBuffer<Luma<u16>, Vec<u16>>;

/// Apply the byte-oriented font mask to the existing unsigned 16-bit luma carrier.
///
/// This preserves the established I;16 text behavior: each destination sample
/// is normalized to 8-bit luma, blended with the text color, and expanded back
/// to the full 16-bit range.
pub(crate) fn apply(
    canvas: &mut I16Image,
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

    for image_y in 0..image_height {
        for image_x in 0..image_width {
            let mask_x = i64::from(image_x) - i64::from(x);
            let mask_y = i64::from(image_y) - i64::from(y);
            let coverage = mask_coverage(mask_x, mask_y, width, height, pixels);
            let sample = canvas.get_pixel(image_x, image_y)[0];
            let background = sample.min(u16::from(u8::MAX)) as u8;
            let luma = blend_luma(background, coverage, fill);
            canvas.put_pixel(image_x, image_y, Luma([u16::from(luma) * 257]));
        }
    }

    true
}

fn mask_coverage(mask_x: i64, mask_y: i64, width: u32, height: u32, pixels: &[u8]) -> u8 {
    if mask_x < 0 || mask_y < 0 || mask_x >= i64::from(width) || mask_y >= i64::from(height) {
        return 0;
    }
    let offset = ((mask_y as u32 * width + mask_x as u32) * 4) as usize;
    pixels[offset + 3]
}

fn blend_luma(background: u8, coverage: u8, fill: (u8, u8, u8, u8)) -> u8 {
    if coverage == 0 {
        return background;
    }
    let inverse = 255u16 - u16::from(coverage);
    Rgb([
        blend_u8(fill.0, background, coverage, inverse),
        blend_u8(fill.1, background, coverage, inverse),
        blend_u8(fill.2, background, coverage, inverse),
    ])
    .to_luma()
    .0[0]
}
