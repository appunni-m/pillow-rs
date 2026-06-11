//! ImageOps — high-level image operations (module-level functions).
//! Mirroring PIL.ImageOps: autocontrast, equalize, invert, flip, mirror,
//! posterize, solarize, expand, scale, contain, cover, fit, pad, grayscale.
//!
//! Pixel-parallel ops use rayon on native targets for multicore speedup.
//! GPU path (GpuEngine methods in src/gpu/) will replace rayon when wired.

use image::{DynamicImage, GenericImage};

use crate::error::PilError;
use crate::image::Image;

/// Apply a per-pixel RGB transform in parallel (native) or sequential (WASM).
/// GPU path will replace this when GpuEngine is wired.
fn par_transform_rgb<F: Fn(u8, u8, u8) -> (u8, u8, u8) + Sync>(
    rgb: &mut image::RgbImage,
    f: F,
) {
    let (w, h) = rgb.dimensions();
    let data = rgb.as_mut_ptr();
    let row_bytes = (w * 3) as usize;
    let total = (w as usize) * (h as usize);

    #[cfg(not(target_arch = "wasm32"))]
    {
        use rayon::prelude::*;
        let slice = unsafe { std::slice::from_raw_parts_mut(data, total * 3) };
        slice.par_chunks_mut(3).for_each(|px| {
            let (r, g, b) = f(px[0], px[1], px[2]);
            px[0] = r; px[1] = g; px[2] = b;
        });
    }

    #[cfg(target_arch = "wasm32")]
    {
        for i in 0..total {
            unsafe {
                let p = data.add(i * 3);
                let (r, g, b) = f((*p), (*p.add(1)), (*p.add(2)));
                *p = r; *p.add(1) = g; *p.add(2) = b;
            }
        }
    }

    let _ = (row_bytes, data);
}

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
    let lo_f = lo as f64;
    let auto = |v: u8| -> u8 { ((v as f64 - lo_f) * scale).clamp(0.0, 255.0) as u8 };
    par_transform_rgb(&mut rgb, move |r, g, b| (auto(r), auto(g), auto(b)));

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
    let cdf_ref = &cdf;
    par_transform_rgb(&mut rgb, |r, g, b| {
        let luma_val = (0.299 * r as f64 + 0.587 * g as f64 + 0.114 * b as f64) as usize;
        let mapped = cdf_ref[luma_val.min(255)] as f64 / 255.0;
        (
            (r as f64 * mapped).clamp(0.0, 255.0) as u8,
            (g as f64 * mapped).clamp(0.0, 255.0) as u8,
            (b as f64 * mapped).clamp(0.0, 255.0) as u8,
        )
    });
    let _ = luma; // moved into closure

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
    par_transform_rgb(&mut rgb, |r, g, b| (255 - r, 255 - g, 255 - b));
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
    par_transform_rgb(&mut rgb, |r, g, b| (r & mask, g & mask, b & mask));
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
    let t = threshold;
    par_transform_rgb(&mut rgb, move |r, g, b| {
        let sol = |v: u8| if v > t { 255 - v } else { v };
        (sol(r), sol(g), sol(b))
    });
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

/// Convert to grayscale using PIL-compatible BT.601 formula.
pub fn grayscale(image: &Image) -> Result<Image, PilError> {
    let mut clone = image.clone();
    let img = clone.ensure_loaded()?;
    Ok(Image {
        inner: crate::lazy::LazyImage::Loaded(image::DynamicImage::ImageLuma8(
            crate::color::pil_grayscale(img),
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
