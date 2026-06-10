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

/// Screen blend mode.
pub fn screen(image1: &Image, image2: &Image) -> Result<Image, PilError> {
    channel_op(image1, image2, |a, b| {
        255u16.saturating_sub((((255.0 - a as f64) * (255.0 - b as f64)) / 255.0).round() as u16) as u8
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

/// Generic per-channel operation helper.
fn channel_op<F>(image1: &Image, image2: &Image, op: F) -> Result<Image, PilError>
where
    F: Fn(u8, u8) -> u8,
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

    let mut result = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p1 = rgb1.get_pixel(x, y);
            let p2 = rgb2.get_pixel(x, y);
            result.put_pixel(
                x,
                y,
                image::Rgb([op(p1[0], p2[0]), op(p1[1], p2[1]), op(p1[2], p2[2])]),
            );
        }
    }

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(DynamicImage::ImageRgb8(result)),
        format: image1.format,
    })
}
