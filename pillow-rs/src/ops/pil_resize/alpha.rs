use crate::raster::DynamicImage;

pub(crate) fn premultiply_alpha(img: &DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgba8(rgba) => {
            let mut out = rgba.clone();
            for p in out.pixels_mut() {
                let a = p[3] as f64 / 255.0;
                p[0] = (p[0] as f64 * a + 0.5) as u8;
                p[1] = (p[1] as f64 * a + 0.5) as u8;
                p[2] = (p[2] as f64 * a + 0.5) as u8;
            }
            DynamicImage::ImageRgba8(out)
        }
        DynamicImage::ImageLumaA8(la) => {
            let mut out = la.clone();
            for p in out.pixels_mut() {
                let a = p[1] as f64 / 255.0;
                p[0] = (p[0] as f64 * a + 0.5) as u8;
            }
            DynamicImage::ImageLumaA8(out)
        }
        _ => img.clone(),
    }
}

pub(crate) fn unpremultiply_rgba(rgba: &crate::raster::RgbaImage) -> crate::raster::RgbaImage {
    let mut out = rgba.clone();
    for p in out.pixels_mut() {
        let Some(alpha) = std::num::NonZeroU32::new(u32::from(p[3])) else {
            continue;
        };
        // Pillow's RGBa -> RGBA conversion uses integer
        // `(255 * value) / alpha` truncation. Keep the multiply
        // before division: using `255.0 / alpha` and a floating
        // multiply can land one ULP below an exact integer.
        p[0] = (u32::from(p[0]) * 255 / alpha.get()).min(255) as u8;
        p[1] = (u32::from(p[1]) * 255 / alpha.get()).min(255) as u8;
        p[2] = (u32::from(p[2]) * 255 / alpha.get()).min(255) as u8;
    }
    out
}

pub(crate) fn unpremultiply_alpha(img: &DynamicImage) -> DynamicImage {
    match img {
        DynamicImage::ImageRgba8(rgba) | DynamicImage::ImageRgbPremultiplied8(rgba) => {
            DynamicImage::ImageRgba8(unpremultiply_rgba(rgba))
        }
        DynamicImage::ImageLumaA8(la) => {
            let mut out = la.clone();
            for p in out.pixels_mut() {
                let Some(alpha) = std::num::NonZeroU32::new(u32::from(p[1])) else {
                    continue;
                };
                // Match Pillow's La -> LA integer truncation and avoid
                // reciprocal floating-point rounding at exact results.
                p[0] = (u32::from(p[0]) * 255 / alpha.get()).min(255) as u8;
            }
            DynamicImage::ImageLumaA8(out)
        }
        _ => img.clone(),
    }
}

#[inline]
pub(super) fn premultiply_channel(value: u8, alpha: u8) -> u8 {
    ((value as u16 * alpha as u16 + 127) / 255) as u8
}

#[cfg(any(not(feature = "parallel"), test))]
pub(super) fn premultiply_alpha_row(source: &[u8], channels: usize, output: &mut [u8]) {
    debug_assert!(matches!(channels, 2 | 4));
    debug_assert_eq!(source.len(), output.len());
    let alpha_channel = channels - 1;
    for (source_pixel, output_pixel) in source
        .chunks_exact(channels)
        .zip(output.chunks_exact_mut(channels))
    {
        let alpha = source_pixel[alpha_channel];
        for channel in 0..alpha_channel {
            output_pixel[channel] = premultiply_channel(source_pixel[channel], alpha);
        }
        output_pixel[alpha_channel] = alpha;
    }
}

/// Reference alpha-row kernel retained for the serial CPU path and focused
/// equivalence tests. Parallel CPU consumes each alpha tap directly per row.
#[inline]
pub(crate) fn unpremultiply_channel(value: u8, alpha: u8) -> u8 {
    if alpha == 0 {
        return value;
    }
    // Both inputs are bytes, so Pillow's f64 expression has an exact
    // integer numerator. Integer division preserves its truncation; clamp
    // because Rust's float-to-u8 cast in the reference path saturates.
    (u16::from(value) * 255 / u16::from(alpha)).min(255) as u8
}
