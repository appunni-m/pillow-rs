//! Pillow `Image` module-level functions.
//!
//! These functions mirror surfaces such as `Image.merge`, `Image.blend`,
//! `Image.composite`, `Image.eval`, and synthetic image effects.

use std::sync::Arc;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;
use crate::ops::convert::parse_mode;
use crate::pipeline::PipelineOp;

/// Merges single-band images into a multi-band image.
///
/// `mode` determines the required band count: `L=1`, `LA=2`, `RGB=3`, and
/// `RGBA=4`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported or `bands` has
/// the wrong length.
pub fn merge(mode: &str, bands: &[Image]) -> Result<Image, PilError> {
    let n_expected = match mode {
        "RGB" => 3,
        "RGBA" => 4,
        "CMYK" => 4,
        "LA" => 2,
        "L" => 1,
        _ => {
            // Pillow 12.2 looks up the mode in ``ImageMode.getmode`` before
            // validating the band sequence, so an unknown mode is surfaced as
            // ``KeyError(mode)`` rather than a generic value error.
            return Err(PilError::KeyError(mode.to_owned()));
        }
    };

    if bands.len() != n_expected {
        return Err(PilError::ValueError("wrong number of bands".into()));
    }

    let mode_enum = parse_mode(mode)?;
    let mut result = Image::push_op(
        &bands[0],
        PipelineOp::Merge {
            mode: mode_enum,
            bands: bands.to_vec(),
        },
    );
    if let Image::Pipeline {
        explicit_mode: tag, ..
    } = &mut result
    {
        if mode == "CMYK" {
            *tag = Some("CMYK".to_string());
        }
    }
    Ok(result)
}

/// Blends two same-sized images by linear interpolation.
///
/// `alpha` is clamped to `0.0..=1.0`; output is `(1 - alpha) * image1 +
/// alpha * image2`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when image dimensions differ, or another
/// [`PilError`] when size lookup fails.
pub fn blend(image1: &Image, image2: &Image, alpha: f64) -> Result<Image, PilError> {
    if image1.mode()? == "P" || image2.mode()? == "P" {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let (w1, h1) = image1.size()?;
    let (w2, h2) = image2.size()?;
    if (w1, h1) != (w2, h2) {
        return Err(PilError::ValueError("images do not match".into()));
    }
    Ok(Image::push_op(
        image1,
        PipelineOp::BlendModule {
            other: Arc::new(image2.clone()),
            alpha,
        },
    ))
}

/// Composites `image1` over `image2` using `mask`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `image1` and `mask` dimensions differ,
/// or another [`PilError`] when size lookup fails.
pub fn composite(image1: &Image, image2: &Image, mask: &Image) -> Result<Image, PilError> {
    // PIL.composite = image2.copy() followed by image2.paste(image1, None, mask)
    // The output matches image2's size; smaller images are pasted at (0,0).
    // PIL requires image1 and mask to have the same size (via paste).
    // image2 can be a different size (it's the background canvas).
    let (w1, h1) = image1.size()?;
    let (wm, hm) = mask.size()?;
    if (w1, h1) != (wm, hm) {
        return Err(PilError::ValueError("images do not match".into()));
    }
    let mask_alpha = match mask.mode()?.as_str() {
        "1" | "L" => false,
        "LA" | "RGBA" | "RGBa" => true,
        _ => return Err(PilError::ValueError("bad transparency mask".into())),
    };
    let output_mode = image2.mode()?;
    let source = if image1.mode()? == output_mode {
        image1.clone()
    } else {
        // Pillow 12.2 PIL.Image.composite starts from image2.copy(), then
        // Image.paste converts image1 to the destination mode before blending.
        image1.convert(&output_mode, None, None, None, None)?
    };
    let mut result = Image::push_op(
        &source,
        PipelineOp::CompositeModule {
            other: Arc::new(image2.clone()),
            mask: Arc::new(mask.clone()),
            mask_alpha,
        },
    );
    if let Image::Pipeline {
        ref mut explicit_mode,
        ref mut palette,
        ref mut palette_alpha,
        ..
    } = result
    {
        *explicit_mode = image2.explicit_mode().map(str::to_owned);
        if output_mode == "P" {
            // The output begins as image2.copy(), so palette ownership follows
            // image2 even when image1 was converted to P for the paste.
            *palette = Some(image2.extract_palette().unwrap_or_default());
            *palette_alpha = image2.palette_alpha();
        } else {
            *palette = None;
            *palette_alpha = None;
        }
    }
    Ok(result)
}

/// Applies a lookup table to each pixel.
///
/// `lut` is copied into the lazy operation and interpreted by the point/eval
/// backend according to the image mode.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; LUT length validation is handled by pipeline
/// execution.
pub fn eval(image: &Image, lut: &[u8]) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image,
        PipelineOp::Eval { lut: lut.to_vec() },
    ))
}

/// Applies a lookup table, expanding a single-band table across bands.
///
/// A 256-entry `lut` is replicated `n_bands` times. A table already sized to
/// `256 * n_bands` is used as-is.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `n_bands` is outside `1..=4` or `lut`
/// has neither 256 nor `256 * n_bands` entries.
pub fn eval_replicated(image: &Image, lut: &[u8], n_bands: usize) -> Result<Image, PilError> {
    if n_bands == 0 || n_bands > 4 {
        return Err(PilError::ValueError("invalid band count".into()));
    }
    let expected = 256 * n_bands;
    if lut.len() == expected {
        return eval(image, lut);
    }
    if lut.len() != 256 {
        return Err(PilError::ValueError(format!(
            "wrong number of lut entries: expected {} or {} got {}",
            256,
            expected,
            lut.len()
        )));
    }
    let mut replicated = Vec::with_capacity(expected);
    for _ in 0..n_bands {
        replicated.extend_from_slice(lut);
    }
    eval(image, &replicated)
}

/// Applies a single-band lookup table to every band in `image`.
///
/// The image mode determines the band count. Bindings should use this entry
/// point instead of deriving semantic image information in the host language.
///
/// # Errors
///
/// Returns an image/materialization error when the band count cannot be read,
/// or [`PilError::ValueError`] when `lut` is not a valid single-band or
/// already-expanded table.
pub fn eval_replicated_for_image(image: &Image, lut: &[u8]) -> Result<Image, PilError> {
    let n_bands = image.getbands()?.len();
    eval_replicated(image, lut, n_bands)
}

/// Validates and applies a pre-expanded Pillow lookup table.
///
/// Pillow requires exactly 256 entries per image band for a non-callable LUT.
/// Keeping this validation in core ensures every ABI observes the same mode
/// semantics and error ordering.
///
/// # Errors
///
/// Returns an image/materialization error when the band count cannot be read,
/// or [`PilError::ValueError`] when the table length does not equal
/// `256 * image_band_count`.
pub fn eval_validated(image: &Image, lut: &[u8]) -> Result<Image, PilError> {
    let n_bands = image.getbands()?.len();
    let expected = 256 * n_bands;
    if lut.len() != expected {
        return Err(PilError::ValueError("wrong number of lut entries".into()));
    }
    eval(image, lut)
}

/// Generates Gaussian noise using the source image dimensions.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn effect_noise(image: &Image, sigma: f64) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::EffectNoise { sigma }))
}

/// Spreads pixels outward by up to `distance` pixels.
///
/// # Errors
///
/// Currently returns `Ok(Image)`; deferred pipeline execution reports later
/// materialization failures.
pub fn effect_spread(image: &Image, distance: u32) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::EffectSpread { distance }))
}

/// Generates a 256 by 256 linear gradient.
///
/// Single-channel modes increase from black at the top to white at the bottom.
/// Supported modes are `"1"`, `"L"`, `"P"`, `"I"`, and `"F"`.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported, or another
/// [`PilError`] when raw image construction fails.
pub fn linear_gradient(mode: &str) -> Result<Image, PilError> {
    if mode.len() != 1 {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let bytes_per_pixel = match mode {
        "L" | "P" => 1,
        "1" => 1,
        "I" => 4,
        "F" => 4,
        _ => {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
    };
    let size: usize = 256 * 256 * bytes_per_pixel;
    let mut data = vec![0u8; size];

    for y in 0..256usize {
        let row_start = y * 256 * bytes_per_pixel;
        match mode {
            "L" | "P" | "1" => {
                let val = y as u8;
                data[row_start..row_start + 256].fill(val);
            }
            "I" => {
                // 4-byte i32 LE per pixel
                let val = y as i32;
                let bytes = val.to_le_bytes();
                for x in 0..256 {
                    let off = row_start + x * 4;
                    data[off..off + 4].copy_from_slice(&bytes);
                }
            }
            "F" => {
                // 4-byte f32 LE per pixel
                let val = y as f32;
                let bytes = val.to_le_bytes();
                for x in 0..256 {
                    let off = row_start + x * 4;
                    data[off..off + 4].copy_from_slice(&bytes);
                }
            }
            _ => unreachable!(),
        }
    }
    Image::frombytes(mode, (256, 256), &data)
}

/// Generates a 256 by 256 radial gradient.
///
/// Supported modes are `"1"`, `"L"`, `"P"`, `"I"`, and `"F"`. Pixel values
/// follow Pillow's radial distance formula.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `mode` is unsupported, or another
/// [`PilError`] when raw image construction fails.
pub fn radial_gradient(mode: &str) -> Result<Image, PilError> {
    if mode.len() != 1 {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    let bytes_per_pixel = match mode {
        "L" | "P" => 1,
        "1" => 1,
        "I" => 4,
        "F" => 4,
        _ => {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
    };
    let size: usize = 256 * 256 * bytes_per_pixel;
    let mut data = vec![0u8; size];

    for y in 0..256 {
        for x in 0..256 {
            let dx = x as f64 - 128.0;
            let dy = y as f64 - 128.0;
            // PIL exact formula: d = (int) sqrt((dx*dx + dy*dy) * 2.0)
            let d = ((dx * dx + dy * dy) * 2.0).sqrt() as i32;
            let val = if d >= 255 { 255u8 } else { d as u8 };

            match mode {
                "L" | "P" | "1" => {
                    data[y * 256 + x] = val;
                }
                "I" => {
                    let bytes = (val as i32).to_le_bytes();
                    let off = (y * 256 + x) * 4;
                    data[off..off + 4].copy_from_slice(&bytes);
                }
                "F" => {
                    let bytes = (val as f32).to_le_bytes();
                    let off = (y * 256 + x) * 4;
                    data[off..off + 4].copy_from_slice(&bytes);
                }
                _ => unreachable!(),
            }
        }
    }
    Image::frombytes(mode, (256, 256), &data)
}

/// Generates a Mandelbrot effect image.
///
/// `size` is output dimensions, `extent` is `(x0, y0, x1, y1)` in the complex
/// plane, and `quality` controls iteration count.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] for zero size, negative extent dimensions,
/// or `quality < 2`. Returns [`PilError::DimensionError`] when allocation
/// checks fail.
pub fn effect_mandelbrot(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<Image, PilError> {
    let (w, h) = size;
    if w == 0 || h == 0 {
        // Pillow accepts a zero-size mandelbrot and returns an empty image.
        return Image::new(w, h, "L", (0, 0, 0, 255));
    }

    let (x0, y0, x1, y1) = extent;
    let width = x1 - x0;
    let height = y1 - y0;

    if width < 0.0 || height < 0.0 || quality < 2 {
        return Err(PilError::ValueError("unrecognized argument value".into()));
    }

    // Pillow's C implementation divides by ``width - 1`` and ``height - 1``
    // without a degenerate-dimension guard.  For a one-pixel axis this yields
    // NaN, and every comparison in the iteration loop remains false, producing
    // the all-zero row/column observed from the public API.  Preserve that
    // version-matched behavior instead of replacing it with a finite stride.
    let dr = width / (w - 1) as f64;
    let di = height / (h - 1) as f64;

    // PIL uses escape radius 100.0 (NOT the common 4.0)
    let radius = 100.0f64;
    let mut data = CheckedDims::new(w, h, 1)?.alloc_buffer();

    for y in 0..h {
        let row_start = (y * w) as usize;
        for x in 0..w {
            let cr = x as f64 * dr + x0;
            let ci = y as f64 * di + y0;

            // PIL's exact loop: for (k = 1;; k++) with check order:
            //   1. compute Mandelbrot iteration
            //   2. check escape → pixel = k*255/quality (as u8, may overflow)
            //   3. check k > quality → pixel = 0 (never escaped)
            let mut zx = 0.0f64;
            let mut zy = 0.0f64;
            let mut zx2 = 0.0f64;
            let mut zy2 = 0.0f64;

            let mut k: i32 = 1;
            loop {
                // y1 = 2 * x1 * y1 + ci
                zy = 2.0 * zx * zy + ci;
                // x1 = xi2 - yi2 + cr  (using OLD xi2/yi2)
                zx = zx2 - zy2 + cr;
                zx2 = zx * zx;
                zy2 = zy * zy;

                if zx2 + zy2 > radius {
                    // PIL: buf[x] = k * 255 / quality (stored as UINT8)
                    // In C: int val = k * 255 / quality; buf[x] = (UINT8)val;
                    let val = (k * 255 / quality) as u8;
                    data[row_start + x as usize] = val;
                    break;
                }
                if k > quality {
                    data[row_start + x as usize] = 0;
                    break;
                }
                k += 1;
            }
        }
    }

    Image::frombytes("L", (w, h), &data)
}
