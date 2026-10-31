//! Native image facts shared by backend enhancement implementations.

use crate::raster::{DynamicImage, Mode};

pub(crate) struct ContrastBase {
    #[cfg(feature = "gpu")]
    pub(crate) mean: u8,
    pub(crate) channels: usize,
    pub(crate) values: [u8; 4],
    pub(crate) alpha: Option<usize>,
}

#[derive(Clone, Copy)]
enum ContrastColorSpace {
    Luma,
    Rgb,
    Cmyk,
    YCbCr,
    Hsv,
}

fn contrast_luma(pixel: &[u8], color_space: ContrastColorSpace) -> u8 {
    match color_space {
        ContrastColorSpace::Luma => pixel[0],
        ContrastColorSpace::Rgb => crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]),
        ContrastColorSpace::Cmyk => {
            let nk = 255 - u32::from(pixel[3]);
            let rgb = |channel: u8| (nk - crate::color::muldiv255(u32::from(channel), nk)) as u8;
            crate::color::rgb_to_luma_u8(rgb(pixel[0]), rgb(pixel[1]), rgb(pixel[2]))
        }
        // Pillow's YCbCr-to-L conversion copies the Y band directly.
        ContrastColorSpace::YCbCr => pixel[0],
        ContrastColorSpace::Hsv => {
            let [red, green, blue] = crate::color::hsv_pixel_to_rgb(pixel[0], pixel[1], pixel[2]);
            crate::color::rgb_to_luma_u8(red, green, blue)
        }
    }
}

/// Native-mode equivalent of convert-to-L, rounded mean, and L-to-source base.
pub(crate) fn contrast_base(img: &DynamicImage) -> Option<ContrastBase> {
    contrast_base_for_mode(img.mode(), img.as_bytes())
}

/// Computes a Contrast base from one exact mode and its native byte samples.
pub(crate) fn contrast_base_for_mode(mode: Mode, source: &[u8]) -> Option<ContrastBase> {
    let (channels, alpha, color_space) = match mode {
        Mode::ImageLuma8 => (1, None, ContrastColorSpace::Luma),
        Mode::ImageLumaA8 => (2, Some(1), ContrastColorSpace::Luma),
        Mode::ImageRgb8 => (3, None, ContrastColorSpace::Rgb),
        Mode::ImageRgba8 => (4, Some(3), ContrastColorSpace::Rgb),
        Mode::ImageRgbx8 => (4, None, ContrastColorSpace::Rgb),
        Mode::ImageCmyk8 => (4, None, ContrastColorSpace::Cmyk),
        Mode::ImageYCbCr8 => (3, None, ContrastColorSpace::YCbCr),
        Mode::ImageHsv8 => (3, None, ContrastColorSpace::Hsv),
        _ => return None,
    };
    let count = source.len() / channels;
    let sum: u64 = source
        .chunks_exact(channels)
        .map(|pixel| u64::from(contrast_luma(pixel, color_space)))
        .sum();
    let mean = if count == 0 {
        0
    } else {
        (sum as f64 / count as f64 + 0.5) as u8
    };
    let values = match color_space {
        ContrastColorSpace::Cmyk => [0, 0, 0, 255 - mean],
        // The constant L-to-YCbCr conversion fixes chroma at its neutral byte.
        ContrastColorSpace::YCbCr => [mean, 128, 128, 255],
        // The constant L-to-HSV conversion represents gray as H=0, S=0, V=L.
        ContrastColorSpace::Hsv => [0, 0, mean, 255],
        ContrastColorSpace::Luma | ContrastColorSpace::Rgb => [mean, mean, mean, 255],
    };
    Some(ContrastBase {
        #[cfg(feature = "gpu")]
        mean,
        channels,
        alpha,
        values,
    })
}
