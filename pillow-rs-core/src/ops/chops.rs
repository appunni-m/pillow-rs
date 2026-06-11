//! ImageChops — channel operations (arithmetic, logical, blending).
//! All functions take two images and return a new combined image.

use image::{DynamicImage, GenericImage, GenericImageView};

use crate::error::PilError;
use crate::image::Image;

/// Add two images. Result = image1 + image2, scaled and offset.
pub fn add(
    image1: &Image,
    image2: &Image,
    scale: f64,
    offset: f64,
) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| {
        ((a as f64 + b as f64) * scale + offset).clamp(0.0, 255.0) as u8
    })
}

/// Subtract image2 from image1.
pub fn subtract(
    image1: &Image,
    image2: &Image,
    scale: f64,
    offset: f64,
) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| {
        ((a as f64 - b as f64) * scale + offset).clamp(0.0, 255.0) as u8
    })
}

/// Multiply two images.
pub fn multiply(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| {
        ((a as f64 * b as f64) / 255.0).round() as u8
    })
}

/// Screen blend mode (PIL uses integer division).
pub fn screen(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| {
        let a_inv = 255u32 - a as u32;
        let b_inv = 255u32 - b as u32;
        (255u32 - (a_inv * b_inv / 255)) as u8
    })
}

/// Return the darker pixel at each position.
pub fn darker(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| a.min(b))
}

/// Return the lighter pixel at each position.
pub fn lighter(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| a.max(b))
}

/// Absolute difference between two images.
pub fn difference(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| (a as i16 - b as i16).unsigned_abs() as u8)
}

/// Overlay blend mode.
pub fn overlay(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |base, blend| {
        let b = base as f64 / 255.0;
        let bl = blend as f64 / 255.0;
        let r = if b < 0.5 {
            2.0 * b * bl
        } else {
            1.0 - 2.0 * (1.0 - b) * (1.0 - bl)
        };
        (r * 255.0).round() as u8
    })
}

/// Soft light blend mode.
pub fn soft_light(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |base, blend| {
        let b = base as f64 / 255.0;
        let bl = blend as f64 / 255.0;
        let r = if bl < 0.5 {
            b - (1.0 - 2.0 * bl) * b * (1.0 - b)
        } else {
            b + (2.0 * bl - 1.0) * ((if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.sqrt() }) - b)
        };
        (r * 255.0).round().clamp(0.0, 255.0) as u8
    })
}

/// Hard light blend mode.
pub fn hard_light(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |base, blend| {
        let bl = blend as f64 / 255.0;
        if bl < 0.5 {
            ((2.0 * base as f64 * bl) / 255.0).round() as u8
        } else {
            255 - ((2.0 * (255.0 - base as f64) * (1.0 - bl)) / 255.0).round() as u8
        }
    })
}

/// Bitwise AND.
pub fn logical_and(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| a & b)
}

/// Bitwise OR.
pub fn logical_or(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| a | b)
}

/// Bitwise XOR.
pub fn logical_xor(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| a ^ b)
}

/// Duplicate an image.
pub fn duplicate(image: &Image) -> Image {
    image.copy()
}

/// Invert an image (same as ImageOps.invert).
pub fn invert(image: &Image) -> Result<Image, PilError> {
    crate::ops::imageops::invert(image)
}

/// Offset image contents.
pub fn offset(image: &Image, xoffset: i32, yoffset: i32) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let (w, h) = (img.width(), img.height());

    let mut result = DynamicImage::new_rgba8(w, h);
    let src_rgba = img.to_rgba8();

    for py in 0..h {
        for px in 0..w {
            let sx = (px as i32 + xoffset).rem_euclid(w as i32) as u32;
            let sy = (py as i32 + yoffset).rem_euclid(h as i32) as u32;
            result.put_pixel(px, py, *src_rgba.get_pixel(sx, sy));
        }
    }

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(result),
        format: image.format,
    })
}

/// Modulo addition (wrap-around).
pub fn add_modulo(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| a.wrapping_add(b))
}

/// Modulo subtraction.
pub fn subtract_modulo(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| a.wrapping_sub(b))
}

/// Fill with constant value (single-channel fill). Parallelized.
pub fn constant(image: &Image, value: u8) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let (w, h) = (img.width(), img.height());
    let total = (w as usize) * (h as usize) * 3;
    let mut out = vec![0u8; total];

    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        out.par_chunks_mut(3).for_each(|px| { px[0] = value; px[1] = value; px[2] = value; });
    }
    #[cfg(target_arch = "wasm32")]
    {
        for px in out.chunks_mut(3) { px[0] = value; px[1] = value; px[2] = value; }
    }

    let out_img = image::RgbImage::from_raw(w, h, out)
        .ok_or_else(|| PilError::ValueError("constant: failed to construct output".into()))?;
    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgb8(out_img)),
        format: image.format,
    })
}

/// Generic per-channel operation helper.
/// Uses rayon for parallel processing on native targets.
fn channel_op<F>(image1: &Image, image2: &Image, op: F) -> Result<Image, PilError>
where
    F: Fn(u8, u8) -> u8 + Sync,
{
    let mut clone1 = image1.clone();
    let mut clone2 = image2.clone();
    let img1 = clone1.ensure_loaded()?;
    let img2 = clone2.ensure_loaded()?;

    let (w, h) = (
        img1.width().min(img2.width()),
        img1.height().min(img2.height()),
    );

    let rgb1 = img1.to_rgb8();
    let rgb2 = img2.to_rgb8();
    let wu = w as usize;
    let hu = h as usize;

    let raw1 = rgb1.as_raw().as_slice();
    let raw2 = rgb2.as_raw().as_slice();
    let mut out = vec![0u8; wu * hu * 3];

    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        out.par_chunks_mut(3).enumerate().for_each(|(i, px)| {
            let idx = i * 3;
            px[0] = op(raw1[idx], raw2[idx]);
            px[1] = op(raw1[idx + 1], raw2[idx + 1]);
            px[2] = op(raw1[idx + 2], raw2[idx + 2]);
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        for i in 0..(wu * hu) {
            let idx = i * 3;
            out[idx] = op(raw1[idx], raw2[idx]);
            out[idx + 1] = op(raw1[idx + 1], raw2[idx + 1]);
            out[idx + 2] = op(raw1[idx + 2], raw2[idx + 2]);
        }
    }

    let result = image::RgbImage::from_raw(w, h, out)
        .ok_or_else(|| PilError::ValueError("channel_op: failed to construct output".into()))?;

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(result)),
        format: image1.format,
    })
}
