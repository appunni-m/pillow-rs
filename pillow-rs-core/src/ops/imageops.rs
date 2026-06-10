//! ImageOps — high-level image operations (module-level functions).
//! Mirroring PIL.ImageOps: autocontrast, equalize, invert, flip, mirror,
//! posterize, solarize, expand, scale, contain, cover, fit, pad, grayscale.

use image::{DynamicImage, GenericImage, GenericImageView};

use crate::error::PilError;
use crate::image::Image;

/// Normalize image contrast. Clips the darkest and lightest `cutoff` percent.
pub fn autocontrast(image: &Image, cutoff: f64) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let gray = img.to_luma8();
    let total = gray.len() as f64;
    let low_thresh = (total * cutoff / 100.0) as usize;
    let high_thresh = (total * (100.0 - cutoff) / 100.0) as usize;

    let mut sorted: Vec<u8> = gray.iter().copied().collect();
    sorted.sort_unstable();

    let lo = *sorted.get(low_thresh).unwrap_or(&0);
    let hi = *sorted.get(high_thresh.min(sorted.len() - 1)).unwrap_or(&255);

    if hi <= lo {
        return Ok(clone);
    }

    let mut rgb = img.to_rgb8();
    let scale = 255.0 / (hi - lo) as f64;
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            let v = (p[c] as f64 - lo as f64) * scale;
            p[c] = v.clamp(0.0, 255.0) as u8;
        }
    }

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgb8(rgb)),
        format: image.format,
    })
}

/// Equalize the image histogram.
pub fn equalize(image: &Image) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let luma = img.to_luma8();

    // Build cumulative histogram
    let mut hist = [0u32; 256];
    for &p in luma.iter() {
        hist[p as usize] += 1;
    }
    let total = luma.len() as f64;
    let mut cdf = [0u8; 256];
    let mut accum = 0u32;
    for (i, &h) in hist.iter().enumerate() {
        accum += h;
        cdf[i] = ((accum as f64 / total) * 255.0).round() as u8;
    }

    let (w, h) = luma.dimensions();
    let mut rgb = img.to_rgb8();
    for (px, lp) in rgb.pixels_mut().zip(luma.pixels()) {
        let mapped = cdf[lp[0] as usize] as f64 / 255.0;
        for c in 0..3 {
            px[c] = (px[c] as f64 * mapped).clamp(0.0, 255.0) as u8;
        }
    }

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgb8(rgb)),
        format: image.format,
    })
}

/// Invert all pixel values (negative).
pub fn invert(image: &Image) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let mut rgb = img.to_rgb8();
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] = 255 - p[c];
        }
    }
    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgb8(rgb)),
        format: image.format,
    })
}

/// Flip image vertically.
pub fn flip(image: &Image) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(img.flipv()),
        format: image.format,
    })
}

/// Mirror image horizontally (same as FLIP_LEFT_RIGHT).
pub fn mirror(image: &Image) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(img.fliph()),
        format: image.format,
    })
}

/// Reduce number of bits per color channel.
pub fn posterize(image: &Image, bits: u8) -> Result<Image, PilError> {
    let bits = bits.clamp(1, 8);
    let mask = !((1u8 << (8 - bits)) - 1);
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let mut rgb = img.to_rgb8();
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] &= mask;
        }
    }
    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgb8(rgb)),
        format: image.format,
    })
}

/// Invert all pixel values above threshold.
pub fn solarize(image: &Image, threshold: u8) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let mut rgb = img.to_rgb8();
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            if p[c] > threshold {
                p[c] = 255 - p[c];
            }
        }
    }
    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageRgb8(rgb)),
        format: image.format,
    })
}

/// Convert to grayscale (faster path than convert("L") for simple cases).
pub fn grayscale(image: &Image) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(
            image::imageops::colorops::grayscale(img),
        )),
        format: image.format,
    })
}

/// Add a border around the image.
pub fn expand(
    image: &Image,
    border: u32,
    fill: (u8, u8, u8, u8),
) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    let (w, h) = (img.width(), img.height());
    let new_w = w + 2 * border;
    let new_h = h + 2 * border;

    let mut expanded = image::DynamicImage::new_rgba8(new_w, new_h);
    // Fill border
    for py in 0..new_h {
        for px in 0..new_w {
            expanded.put_pixel(
                px,
                py,
                image::Rgba([fill.0, fill.1, fill.2, fill.3]),
            );
        }
    }
    // Overlay original image in center
    image::imageops::overlay(
        &mut expanded,
        &img.to_rgba8(),
        border as i64,
        border as i64,
    );

    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(expanded),
        format: image.format,
    })
}
