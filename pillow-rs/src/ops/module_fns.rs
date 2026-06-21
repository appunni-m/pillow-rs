//! Image module-level functions — merge, blend, composite, eval, and effects.
//! These correspond to PIL.Image.merge(), PIL.Image.blend(), PIL.Image.composite().

use std::sync::Arc;

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::Image;
use crate::ops::convert::parse_mode;
use crate::pipeline::PipelineOp;

/// Merge single-band images into a multi-band image.
/// PIL: `Image.merge(mode, bands)` where mode determines the band count.
pub fn merge(mode: &str, bands: &[Image]) -> Result<Image, PilError> {
    let n_expected = match mode {
        "RGB" => 3,
        "RGBA" => 4,
        "LA" => 2,
        "L" => 1,
        _ => {
            return Err(PilError::ValueError(format!(
                "Unsupported merge mode: {}",
                mode
            )))
        }
    };

    if bands.len() != n_expected {
        return Err(PilError::ValueError(format!(
            "Wrong number of bands for mode {}: expected {}, got {}",
            mode,
            n_expected,
            bands.len()
        )));
    }

    let mode_enum = parse_mode(mode)?;
    Ok(Image::push_op(
        &bands[0],
        PipelineOp::Merge {
            mode: mode_enum,
            bands: bands.to_vec(),
        },
    ))
}

/// Linear interpolation between two images.
/// PIL: `Image.blend(im1, im2, alpha)` -> (1-alpha)*im1 + alpha*im2
pub fn blend(image1: &Image, image2: &Image, alpha: f64) -> Result<Image, PilError> {
    let alpha = alpha.clamp(0.0, 1.0);
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

/// Composite image1 over image2 using mask.
/// PIL: `Image.composite(image1, image2, mask)`
/// `mode` is an optional mode override (e.g. "P" when composite is called on P-mode images).
pub fn composite(
    image1: &Image,
    image2: &Image,
    mask: &Image,
    mode: Option<&str>,
) -> Result<Image, PilError> {
    // PIL.composite = image2.copy() followed by image2.paste(image1, None, mask)
    // The output matches image2's size; smaller images are pasted at (0,0).
    // PIL requires image1 and mask to have the same size (via paste).
    // image2 can be a different size (it's the background canvas).
    let (w1, h1) = image1.size()?;
    let (wm, hm) = mask.size()?;
    if (w1, h1) != (wm, hm) {
        return Err(PilError::ValueError("images do not match".into()));
    }
    let mut result = Image::push_op(
        image1,
        PipelineOp::CompositeModule {
            other: Arc::new(image2.clone()),
            mask: Arc::new(mask.clone()),
        },
    );
    // Override explicit_mode if provided (e.g., for P-mode images where the
    // Python wrapper stores the mode externally on the Python Image object).
    if let Some(m) = mode {
        if let Image::Pipeline {
            ref mut explicit_mode,
            ..
        } = result
        {
            *explicit_mode = Some(m.to_string());
        }
    }
    Ok(result)
}

/// Apply a lookup table to each pixel.
/// PIL: `Image.eval(image, lut)`
pub fn eval(image: &Image, lut: &[u8]) -> Result<Image, PilError> {
    Ok(Image::push_op(
        image,
        PipelineOp::Eval { lut: lut.to_vec() },
    ))
}

/// Like eval() but replicates a 256-entry LUT across n_bands channels.
/// PIL: `self.point(lut)` where lut is a function → 256*bands entries.
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

/// Generate an image with Gaussian noise.
/// PIL: `Image.effect_noise(size, sigma)`
/// Uses the source image only for dimensions.
pub fn effect_noise(image: &Image, sigma: f64) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::EffectNoise { sigma }))
}

/// Spread pixels outward (visual effect).
/// Uses the source image and applies distance-based spread.
pub fn effect_spread(image: &Image, distance: u32) -> Result<Image, PilError> {
    Ok(Image::push_op(image, PipelineOp::EffectSpread { distance }))
}

/// Generate a 256×256 linear gradient from black (top) to white (bottom).
/// PIL: `Image.linear_gradient(mode)` — only single-channel modes like "L".
/// Matches PIL's ImagingFillLinearGradient in src/libImaging/Fill.c exactly.
pub fn linear_gradient(mode: &str) -> Result<Image, PilError> {
    if mode.len() != 1 {
        return Err(PilError::ValueError(format!(
            "linear_gradient: unsupported mode '{}', only single-channel modes supported",
            mode
        )));
    }
    let bytes_per_pixel = match mode {
        "L" | "P" => 1,
        "1" => 1,
        "I" => 4,
        "F" => 4,
        _ => {
            return Err(PilError::ValueError(format!(
                "linear_gradient: unsupported mode '{}'",
                mode
            )));
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

/// Generate a 256×256 radial gradient from white (center) to black (edges).
/// PIL: `Image.radial_gradient(mode)` — only single-channel modes like "L".
/// Matches PIL's ImagingFillRadialGradient in src/libImaging/Fill.c exactly.
pub fn radial_gradient(mode: &str) -> Result<Image, PilError> {
    if mode.len() != 1 {
        return Err(PilError::ValueError(format!(
            "radial_gradient: unsupported mode '{}', only single-channel modes supported",
            mode
        )));
    }
    let bytes_per_pixel = match mode {
        "L" | "P" => 1,
        "1" => 1,
        "I" => 4,
        "F" => 4,
        _ => {
            return Err(PilError::ValueError(format!(
                "radial_gradient: unsupported mode '{}'",
                mode
            )));
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

/// Generate a Mandelbrot set image.
/// PIL: `Image.effect_mandelbrot(size, extent, quality)`.
/// Matches PIL's ImagingEffectMandelbrot in src/libImaging/Effects.c exactly,
/// including the UINT8 overflow behavior when k > quality and pixel escapes.
pub fn effect_mandelbrot(
    size: (u32, u32),
    extent: (f64, f64, f64, f64),
    quality: i32,
) -> Result<Image, PilError> {
    let (w, h) = size;
    if w == 0 || h == 0 {
        return Err(PilError::ValueError(
            "effect_mandelbrot: size must be > 0".into(),
        ));
    }

    let (x0, y0, x1, y1) = extent;
    let width = x1 - x0;
    let height = y1 - y0;

    if width < 0.0 || height < 0.0 || quality < 2 {
        return Err(PilError::ValueError(
            "effect_mandelbrot: invalid extent or quality".into(),
        ));
    }

    // PIL's exact stride computation
    let dr = if w > 1 { width / (w - 1) as f64 } else { 0.0 };
    let di = if h > 1 { height / (h - 1) as f64 } else { 0.0 };

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
