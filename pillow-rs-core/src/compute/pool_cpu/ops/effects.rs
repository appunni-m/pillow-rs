// ── Effects + Module fns + Point + Mutating operations extracted from image.rs execute_op() ──

use crate::error::PilError;
use crate::image::{preserve_mode, Image};
use crate::pipeline::{ColorMode, ResampleFilter, TransformMethod};
use image::{DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use std::sync::Arc;

// ── glibc-compatible PRNG ────────────────────────────────────────────────
//
// Implements glibc's `srand()`/`rand()` (TYPE_3 algorithm) so PIL's
// deterministic seeded output is reproducible on WASM where libc is absent.
// Verified: with seed 42 it produces the exact same sequence as glibc.

struct GlibcRand {
    state: [i32; 31],
    fptr: usize,
    rptr: usize,
}

impl GlibcRand {
    fn new(seed: u32) -> Self {
        let mut state = [0i32; 31];
        state[0] = (seed & 0x7fffffff) as i32;
        for i in 1..31 {
            state[i] = ((state[i - 1] as i64).wrapping_mul(16807) % 2147483647) as i32;
        }
        let mut rng = GlibcRand {
            state,
            fptr: 3,
            rptr: 0,
        };
        // Warm-up: 310 iterations with pointer advancement (matching glibc)
        for _ in 0..310 {
            rng.advance();
        }
        rng
    }

    /// Core step: state[fptr] += state[rptr], return (val >> 1) & 0x7fffffff
    fn advance(&mut self) -> i32 {
        let val = self.state[self.fptr].wrapping_add(self.state[self.rptr]);
        self.state[self.fptr] = val;
        self.fptr = (self.fptr + 1) % 31;
        self.rptr = (self.rptr + 1) % 31;
        (val >> 1) & 0x7fffffff
    }

    fn next(&mut self) -> i32 {
        self.advance()
    }
}

// ── EffectSpread ──

pub fn op_effect_spread(img: &DynamicImage, distance: u32) -> Result<DynamicImage, PilError> {
    // PIL's ImagingEffectSpread:
    // For image8 (L, P, 1): 1 byte per pixel, SPREAD(UINT8, image8)
    // For image32 (RGB, RGBA, etc): 4 bytes per pixel, SPREAD(INT32, image32)
    // Creates a new output image. For each pixel (x,y) in the input:
    //   Compute (xx,yy) = (x + rand()%d - d/2, y + rand()%d - d/2)
    //   If (xx,yy) is in bounds:
    //     output[yy][xx] = input[y][x]
    //     output[y][x] = input[yy][xx]
    //   Else:
    //     output[y][x] = input[y][x]
    // Input is NEVER modified; output is a new image.
    // Multiple pixels CAN map to the same (xx,yy); last write wins.
    if distance == 0 {
        return Ok(img.clone());
    }
    let d = distance as i32;
    let half_d = d / 2;
    // Determine pixel stride based on color type (PIL uses image8 for L/LA/P with pixelsize,
    // image32 for RGB/RGBA/CMYK with 4-byte stride)
    let (pixels, w, h, stride) = match img.color() {
        image::ColorType::L8 => {
            let luma = img.to_luma8();
            let (w, h) = luma.dimensions();
            (luma.into_raw(), w as i32, h as i32, 1usize)
        }
        image::ColorType::La8 | image::ColorType::La16 => {
            let la = img.to_luma_alpha8();
            let (w, h) = la.dimensions();
            (la.into_raw(), w as i32, h as i32, 2usize)
        }
        image::ColorType::Rgb8 => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            (rgb.into_raw(), w as i32, h as i32, 3usize)
        }
        _ => {
            // RGBA8, or any other 4-channel mode
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            (rgba.into_raw(), w as i32, h as i32, 4usize)
        }
    };
    let input_pixels = pixels;
    let mut out_pixels = input_pixels.clone();

    // Use glibc-compatible PRNG (works on ALL platforms including WASM)
    let mut rng = GlibcRand::new(42);
    for y in 0..h {
        for x in 0..w {
            let src_idx = (y * w + x) as usize;
            let src_base = src_idx * stride;
            let xx = x + (rng.next() % d) - half_d;
            let yy = y + (rng.next() % d) - half_d;
            if xx >= 0 && xx < w && yy >= 0 && yy < h {
                let dst_idx = (yy * w + xx) as usize;
                let dst_base = dst_idx * stride;
                // Read from INPUT (never modified), write to OUTPUT
                out_pixels[dst_base..dst_base + stride]
                    .copy_from_slice(&input_pixels[src_base..src_base + stride]);
                out_pixels[src_base..src_base + stride]
                    .copy_from_slice(&input_pixels[dst_base..dst_base + stride]);
            } else {
                // Copy pixel as-is
                out_pixels[src_base..src_base + stride]
                    .copy_from_slice(&input_pixels[src_base..src_base + stride]);
            }
        }
    }
    // Reconstruct DynamicImage from the output pixel data
    let result = match stride {
        1 => DynamicImage::ImageLuma8(
            GrayImage::from_raw(w as u32, h as u32, out_pixels).ok_or_else(|| {
                PilError::ImageError(image::ImageError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "effect_spread buffer error",
                )))
            })?,
        ),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(w as u32, h as u32, out_pixels).ok_or_else(|| {
                PilError::ImageError(image::ImageError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "effect_spread buffer error",
                )))
            })?,
        ),
        3 => DynamicImage::ImageRgb8(
            RgbImage::from_raw(w as u32, h as u32, out_pixels).ok_or_else(|| {
                PilError::ImageError(image::ImageError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "effect_spread buffer error",
                )))
            })?,
        ),
        _ => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(w as u32, h as u32, out_pixels).ok_or_else(|| {
                PilError::ImageError(image::ImageError::from(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "effect_spread buffer error",
                )))
            })?,
        ),
    };
    Ok(result)
}

// ── Paste ──

pub fn op_paste(
    img: &DynamicImage,
    source: &Arc<Image>,
    x: i64,
    y: i64,
    mask: &Option<Arc<Image>>,
) -> Result<DynamicImage, PilError> {
    let src_img = source.materialize()?;
    let (src_w, src_h) = (src_img.width(), src_img.height());
    let paste_x = x;
    let paste_y = y;

    if let Some(mask_img_ref) = mask {
        let mask_img = mask_img_ref.materialize()?;
        let mask_gray = mask_img.to_luma8();
        let mut dest_clone = img.to_rgba8();

        for py in 0..src_h.min(dest_clone.height()) {
            for px in 0..src_w.min(dest_clone.width()) {
                let mask_val = if px < mask_gray.width() && py < mask_gray.height() {
                    mask_gray.get_pixel(px, py)[0]
                } else {
                    0
                };
                if mask_val == 0 {
                    continue;
                }
                let sp = src_img.get_pixel(px, py);
                let dx = (paste_x + px as i64) as u32;
                let dy = (paste_y + py as i64) as u32;
                if dx >= dest_clone.width() || dy >= dest_clone.height() {
                    continue;
                }
                if mask_val == 255 {
                    dest_clone.put_pixel(dx, dy, sp);
                } else {
                    let inv_alpha = 255u16 - mask_val as u16;
                    let dp = dest_clone.get_pixel(dx, dy);
                    let a = sp.0.get(3).copied().unwrap_or(255) as u16;
                    let da = dp.0.get(3).copied().unwrap_or(255) as u16;
                    let blended = image::Rgba([
                        ((sp[0] as u16 * mask_val as u16 + dp[0] as u16 * inv_alpha + 127) / 255)
                            as u8,
                        ((sp[1] as u16 * mask_val as u16 + dp[1] as u16 * inv_alpha + 127) / 255)
                            as u8,
                        ((sp[2] as u16 * mask_val as u16 + dp[2] as u16 * inv_alpha + 127) / 255)
                            as u8,
                        ((a * mask_val as u16 + da * inv_alpha + 127) / 255) as u8,
                    ]);
                    dest_clone.put_pixel(dx, dy, blended);
                }
            }
        }
        Ok(preserve_mode(img, DynamicImage::ImageRgba8(dest_clone)))
    } else {
        let mut dest_clone = img.to_rgba8();
        image::imageops::overlay(&mut dest_clone, &src_img.to_rgba8(), paste_x, paste_y);
        Ok(preserve_mode(img, DynamicImage::ImageRgba8(dest_clone)))
    }
}

// ── AlphaComposite ──

pub fn op_alpha_composite(
    img: &DynamicImage,
    source: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    let src_img = source.materialize()?;
    let mut dest_rgba = img.to_rgba8();
    let src_rgba = src_img.to_rgba8();
    let (sw, sh) = src_rgba.dimensions();
    for py in 0..sh.min(dest_rgba.height()) {
        for px in 0..sw.min(dest_rgba.width()) {
            let sp = src_rgba.get_pixel(px, py);
            let dp = dest_rgba.get_pixel(px, py);
            let sa = sp[3] as f64 / 255.0;
            let da = dp[3] as f64 / 255.0;
            let out_a = sa + da * (1.0 - sa);
            if out_a <= 0.0 {
                continue;
            }
            let r = ((sp[0] as f64 * sa + dp[0] as f64 * da * (1.0 - sa)) / out_a)
                .round()
                .clamp(0.0, 255.0) as u8;
            let g = ((sp[1] as f64 * sa + dp[1] as f64 * da * (1.0 - sa)) / out_a)
                .round()
                .clamp(0.0, 255.0) as u8;
            let b = ((sp[2] as f64 * sa + dp[2] as f64 * da * (1.0 - sa)) / out_a)
                .round()
                .clamp(0.0, 255.0) as u8;
            let a = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
            dest_rgba.put_pixel(px, py, image::Rgba([r, g, b, a]));
        }
    }
    Ok(DynamicImage::ImageRgba8(dest_rgba))
}

// ── Merge ──

pub fn op_merge(
    img: &DynamicImage,
    mode: &ColorMode,
    bands: &[Arc<Image>],
) -> Result<DynamicImage, PilError> {
    let _n_expected = match mode {
        ColorMode::RGB => 3,
        ColorMode::RGBA => 4,
        ColorMode::LA => 2,
        ColorMode::L | ColorMode::Mode1 => 1,
        _ => {
            return Err(PilError::ValueError(format!(
                "Unsupported merge mode: {:?}",
                mode
            )))
        }
    };
    // Get pixel data from each band
    let mut band_pixels: Vec<Vec<u8>> = Vec::new();
    // First band is the current image
    let first_gray = img.to_luma8();
    let (w, h) = first_gray.dimensions();
    band_pixels.push(first_gray.into_raw());
    for band in bands.iter().skip(1) {
        let b_img = band.materialize()?;
        let b_gray = b_img.to_luma8();
        band_pixels.push(b_gray.into_raw());
    }
    let n = (w * h) as usize;
    match mode {
        ColorMode::RGB => {
            let mut rgb = vec![0u8; n * 3];
            for i in 0..n {
                rgb[i * 3] = band_pixels[0][i];
                rgb[i * 3 + 1] = band_pixels[1][i];
                rgb[i * 3 + 2] = band_pixels[2][i];
            }
            let img = RgbImage::from_raw(w, h, rgb)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgb8(img))
        }
        ColorMode::RGBA => {
            let mut rgba = vec![0u8; n * 4];
            for i in 0..n {
                rgba[i * 4] = band_pixels[0][i];
                rgba[i * 4 + 1] = band_pixels[1][i];
                rgba[i * 4 + 2] = band_pixels[2][i];
                rgba[i * 4 + 3] = band_pixels[3][i];
            }
            let img = RgbaImage::from_raw(w, h, rgba)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(img))
        }
        ColorMode::LA => {
            let mut la = vec![0u8; n * 2];
            for i in 0..n {
                la[i * 2] = band_pixels[0][i];
                la[i * 2 + 1] = band_pixels[1][i];
            }
            let img = GrayAlphaImage::from_raw(w, h, la)
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageLumaA8(img))
        }
        ColorMode::L | ColorMode::Mode1 => {
            let img = GrayImage::from_raw(w, h, band_pixels.remove(0))
                .ok_or_else(|| PilError::ValueError("merge: buffer error".into()))?;
            Ok(DynamicImage::ImageLuma8(img))
        }
        _ => Err(PilError::ValueError("Unsupported merge mode".into())),
    }
}

// ── BlendModule ──

pub fn op_blend_module(
    img: &DynamicImage,
    other: &Arc<Image>,
    alpha: f64,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize()?;
    let a = alpha.clamp(0.0, 1.0);
    // CMYK mode: blend all 4 channels (C,M,Y,K stored as R,G,B,A in Rgba8)
    if explicit_mode == Some("CMYK") {
        let rgba1 = img.to_rgba8();
        let rgba2 = other_img.to_rgba8();
        let (w, h) = (
            rgba1.width().min(rgba2.width()),
            rgba1.height().min(rgba2.height()),
        );
        let mut out = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p1 = rgba1.get_pixel(x, y);
                let p2 = rgba2.get_pixel(x, y);
                out.put_pixel(
                    x,
                    y,
                    image::Rgba([
                        (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                        (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                        (p1[2] as f64 * (1.0 - a) + p2[2] as f64 * a) as u8,
                        (p1[3] as f64 * (1.0 - a) + p2[3] as f64 * a) as u8,
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageRgba8(out));
    }
    let rgb1 = img.to_rgb8();
    let rgb2 = other_img.to_rgb8();
    let (w, h) = (
        rgb1.width().min(rgb2.width()),
        rgb1.height().min(rgb2.height()),
    );
    let mut out = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p1 = rgb1.get_pixel(x, y);
            let p2 = rgb2.get_pixel(x, y);
            out.put_pixel(
                x,
                y,
                image::Rgb([
                    (p1[0] as f64 * (1.0 - a) + p2[0] as f64 * a) as u8,
                    (p1[1] as f64 * (1.0 - a) + p2[1] as f64 * a) as u8,
                    (p1[2] as f64 * (1.0 - a) + p2[2] as f64 * a) as u8,
                ]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

// ── CompositeModule ──

pub fn op_composite_module(
    img: &DynamicImage,
    other: &Arc<Image>,
    mask: &Arc<Image>,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize()?;
    let mask_img = mask.materialize()?;
    // CMYK mode: composite all 4 channels (C,M,Y,K stored as R,G,B,A in Rgba8)
    if explicit_mode == Some("CMYK") {
        let rgba1 = img.to_rgba8();
        let rgba2 = other_img.to_rgba8();
        let mask_gray = mask_img.to_luma8();
        let (w, h) = (
            rgba1.width().min(rgba2.width()).min(mask_gray.width()),
            rgba1.height().min(rgba2.height()).min(mask_gray.height()),
        );
        let mut out = RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p1 = rgba1.get_pixel(x, y);
                let p2 = rgba2.get_pixel(x, y);
                let m = mask_gray.get_pixel(x, y)[0] as f64 / 255.0;
                out.put_pixel(
                    x,
                    y,
                    image::Rgba([
                        (p1[0] as f64 * m + p2[0] as f64 * (1.0 - m)).round() as u8,
                        (p1[1] as f64 * m + p2[1] as f64 * (1.0 - m)).round() as u8,
                        (p1[2] as f64 * m + p2[2] as f64 * (1.0 - m)).round() as u8,
                        (p1[3] as f64 * m + p2[3] as f64 * (1.0 - m)).round() as u8,
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageRgba8(out));
    }
    let rgb1 = img.to_rgb8();
    let rgb2 = other_img.to_rgb8();
    let mask_gray = mask_img.to_luma8();
    let (w, h) = (
        rgb1.width().min(rgb2.width()).min(mask_gray.width()),
        rgb1.height().min(rgb2.height()).min(mask_gray.height()),
    );
    let mut out = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p1 = rgb1.get_pixel(x, y);
            let p2 = rgb2.get_pixel(x, y);
            let m = mask_gray.get_pixel(x, y)[0] as f64 / 255.0;
            out.put_pixel(
                x,
                y,
                image::Rgb([
                    ((p1[0] as f64 * m + p2[0] as f64 * (1.0 - m)).round()) as u8,
                    ((p1[1] as f64 * m + p2[1] as f64 * (1.0 - m)).round()) as u8,
                    ((p1[2] as f64 * m + p2[2] as f64 * (1.0 - m)).round()) as u8,
                ]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

// ── Eval ──

pub fn op_eval(img: &DynamicImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    let n_bands = match img.color() {
        image::ColorType::L8 | image::ColorType::L16 => 1,
        image::ColorType::La8 | image::ColorType::La16 => 2,
        image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
        _ => 4,
    };
    let band_luts: Vec<&[u8]> = if lut.len() >= 256 * n_bands {
        (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect()
    } else {
        vec![lut; n_bands]
    };
    // For single-channel images (mode "1", "L", "P"), operate on Luma8 directly
    // to avoid precision loss through RGBA round-trip.
    if n_bands == 1 {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            let idx = ip[0] as usize;
            op[0] = *band_luts[0].get(idx).unwrap_or(&ip[0]);
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbaImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        for b in 0..4 {
            let idx = ip[b] as usize;
            let band = b.min(band_luts.len() - 1);
            op[b] = *band_luts[band].get(idx).unwrap_or(&ip[b]);
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgba8(out)))
}

// ── EffectNoise ──

pub fn op_effect_noise(img: &DynamicImage, sigma: f64) -> Result<DynamicImage, PilError> {
    // PIL's ImagingEffectNoise: Box-Muller polar transform (gaussian noise).
    // Always produces L mode output. Uses libc rand().
    // This must exactly match PIL's C implementation to produce
    // bit-identical output with the same rand seed.
    //
    // NOTE: The installed PIL 12.2.0 binary does NOT use the Box-Muller
    // caching optimization shown in the GitHub source. It calls rand()
    // twice for EVERY pixel (one Box-Muller pair per pixel, discarding
    // the second value from the pair).
    let (w, h) = (img.width(), img.height());
    let mut out = GrayImage::new(w, h);
    // Use glibc-compatible PRNG on ALL platforms
    let mut rng = GlibcRand::new(42);
    // RAND_MAX on glibc
    const RAND_MAX_F64: f64 = 2147483647.0;
    for pixel in out.pixels_mut() {
        let (v1, radius) = loop {
            // Exact match to PIL:
            //   v1 = rand() * (2.0 / RAND_MAX) - 1.0;
            //   v2 = rand() * (2.0 / RAND_MAX) - 1.0;
            let v1 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
            let v2 = rng.next() as f64 * (2.0 / RAND_MAX_F64) - 1.0;
            let radius = v1 * v1 + v2 * v2;
            if radius < 1.0 {
                break (v1, radius);
            }
        };
        // factor = sqrt(-2.0 * log(radius) / radius)
        let factor = (-2.0 * radius.ln() / radius).sqrt();
        let this = factor * v1;
        // PIL: CLIP8(128 + sigma * this)
        // CLIP8: (v) <= 0 ? 0 : (v) >= 255.0 ? 255 : (UINT8)(v)
        // Cast truncates toward zero (no rounding).
        let v = 128.0 + sigma * this;
        pixel[0] = if v <= 0.0 {
            0
        } else if v >= 255.0 {
            255
        } else {
            v as u8
        };
    }
    Ok(DynamicImage::ImageLuma8(out))
}

// ── PointOp (lookup table) ──

pub fn op_point(img: &DynamicImage, lut: &[u8]) -> Result<DynamicImage, PilError> {
    let n_bands = match img.color() {
        image::ColorType::L8 | image::ColorType::L16 => 1,
        image::ColorType::La8 | image::ColorType::La16 => 2,
        image::ColorType::Rgb8 | image::ColorType::Rgb16 => 3,
        _ => 4,
    };
    // Per-band LUTs: if lut has 256*n_bands entries, split into per-band segments
    let band_luts: Vec<&[u8]> = if lut.len() >= 256 * n_bands {
        (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect()
    } else {
        // Single LUT: apply same to all bands
        vec![lut; n_bands]
    };
    // For single-channel images, operate on Luma8 directly
    // to avoid precision loss through RGBA round-trip.
    if n_bands == 1 {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            let idx = ip[0] as usize;
            op[0] = *band_luts[0].get(idx).unwrap_or(&ip[0]);
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut out = RgbaImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgba.pixels()) {
        for b in 0..4 {
            let idx = ip[b] as usize;
            let band = b.min(band_luts.len() - 1);
            op[b] = *band_luts[band].get(idx).unwrap_or(&ip[b]);
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgba8(out)))
}

// ── Transform ──

/// Apply an affine transform working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling.
fn transform_affine_generic(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    aff_a: f64,
    aff_b: f64,
    aff_c: f64,
    aff_d: f64,
    aff_e: f64,
    aff_f: f64,
    fill: Option<(u8, u8, u8, u8)>,
    nearest: bool,
) -> DynamicImage {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let fill_color = fill.unwrap_or((0, 0, 0, 255));

    let mut out = vec![0u8; (dst_w * dst_h) as usize * channels];

    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let sx = aff_a * dx as f64 + aff_b * dy as f64 + aff_c;
            let sy = aff_d * dx as f64 + aff_e * dy as f64 + aff_f;
            let out_idx = (dy * dst_w + dx) as usize * channels;

            if nearest {
                let ix = (sx + 0.5).floor() as i64;
                let iy = (sy + 0.5).floor() as i64;
                if ix >= 0 && ix < sw as i64 && iy >= 0 && iy < sh as i64 {
                    let in_idx = (iy as u32 * sw + ix as u32) as usize * channels;
                    out[out_idx..out_idx + channels]
                        .copy_from_slice(&raw[in_idx..in_idx + channels]);
                } else {
                    for ch in 0..channels.min(4) {
                        out[out_idx + ch] = match ch {
                            0 => fill_color.0,
                            1 => fill_color.1,
                            2 => fill_color.2,
                            _ => fill_color.3,
                        };
                    }
                }
            } else if sx >= 0.0 && sx < sw as f64 && sy >= 0.0 && sy < sh as f64 {
                let x0 = sx.floor() as u32;
                let y0 = sy.floor() as u32;
                let x1 = (x0 + 1).min(sw - 1);
                let y1 = (y0 + 1).min(sh - 1);
                let fx = sx - x0 as f64;
                let fy = sy - y0 as f64;
                for ch in 0..channels {
                    let p00 = raw[(y0 * sw + x0) as usize * channels + ch] as f64;
                    let p10 = raw[(y0 * sw + x1) as usize * channels + ch] as f64;
                    let p01 = raw[(y1 * sw + x0) as usize * channels + ch] as f64;
                    let p11 = raw[(y1 * sw + x1) as usize * channels + ch] as f64;
                    let v = (1.0 - fx) * (1.0 - fy) * p00
                        + fx * (1.0 - fy) * p10
                        + (1.0 - fx) * fy * p01
                        + fx * fy * p11;
                    out[out_idx + ch] = v.round() as u8;
                }
            } else {
                for ch in 0..channels.min(4) {
                    out[out_idx + ch] = match ch {
                        0 => fill_color.0,
                        1 => fill_color.1,
                        2 => fill_color.2,
                        _ => fill_color.3,
                    };
                }
            }
        }
    }

    match channels {
        1 => DynamicImage::ImageLuma8(
            GrayImage::from_raw(dst_w, dst_h, out).expect("transform_affine: buffer size mismatch"),
        ),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(dst_w, dst_h, out)
                .expect("transform_affine: buffer size mismatch"),
        ),
        3 => DynamicImage::ImageRgb8(
            RgbImage::from_raw(dst_w, dst_h, out).expect("transform_affine: buffer size mismatch"),
        ),
        4 => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(dst_w, dst_h, out).expect("transform_affine: buffer size mismatch"),
        ),
        _ => unreachable!(),
    }
}

pub fn op_transform(
    img: &DynamicImage,
    w: u32,
    h: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: &ResampleFilter,
    fill: Option<(u8, u8, u8, u8)>,
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    match method {
        TransformMethod::Affine => {
            if data.len() < 6 {
                return Err(PilError::ValueError(
                    "Affine transform needs 6 coefficients".into(),
                ));
            }
            let (aff_a, aff_b, aff_c, aff_d, aff_e, aff_f) =
                (data[0], data[1], data[2], data[3], data[4], data[5]);
            let p_mode = explicit_mode == Some("P") || explicit_mode == Some("1");
            let i_f_mode = explicit_mode == Some("I") || explicit_mode == Some("F");
            let use_nearest = matches!(filter, ResampleFilter::Nearest) || p_mode || i_f_mode;

            let result = transform_affine_generic(
                img,
                w,
                h,
                aff_a,
                aff_b,
                aff_c,
                aff_d,
                aff_e,
                aff_f,
                fill,
                use_nearest,
            );
            Ok(preserve_mode(img, result))
        }
        &TransformMethod::Perspective | &TransformMethod::Quad | &TransformMethod::Mesh => Err(
            PilError::NotImplementedError(format!("Transform {:?} not yet implemented", method)),
        ),
    }
}

// ── PutPixel ──

pub fn op_put_pixel(
    img: &DynamicImage,
    x: u32,
    y: u32,
    color: (u8, u8, u8, u8),
) -> Result<DynamicImage, PilError> {
    let mut rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    if x >= w || y >= h {
        return Err(PilError::ValueError(format!(
            "pixel ({},{}) out of bounds ({}x{})",
            x, y, w, h
        )));
    }
    rgba.put_pixel(x, y, image::Rgba([color.0, color.1, color.2, color.3]));
    Ok(preserve_mode(img, DynamicImage::ImageRgba8(rgba)))
}

// ── PutData ──

pub fn op_put_data(img: &DynamicImage, data: &[u8]) -> Result<DynamicImage, PilError> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let expected = match img.color() {
        image::ColorType::L8 => w * h,
        image::ColorType::La8 => w * h * 2,
        image::ColorType::Rgb8 => w * h * 3,
        _ => w * h * 4,
    };
    if data.len() < expected {
        return Err(PilError::ValueError(format!(
            "putdata: expected {} bytes, got {}",
            expected,
            data.len()
        )));
    }
    match img.color() {
        image::ColorType::Rgb8 => {
            let rgb = RgbImage::from_raw(w as u32, h as u32, data[..expected].to_vec())
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        image::ColorType::L8 => {
            let gray = GrayImage::from_raw(w as u32, h as u32, data[..expected].to_vec())
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageLuma8(gray))
        }
        _ => {
            let rgba = RgbaImage::from_raw(w as u32, h as u32, data[..expected].to_vec())
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(rgba))
        }
    }
}

// ── PutAlpha ──

pub fn op_put_alpha(img: &DynamicImage, alpha: u8) -> DynamicImage {
    let out = match img.color() {
        image::ColorType::L8 => {
            let luma = img.to_luma8();
            let mut la = GrayAlphaImage::new(luma.width(), luma.height());
            for (o, i) in la.pixels_mut().zip(luma.pixels()) {
                o[0] = i[0];
                o[1] = alpha;
            }
            DynamicImage::ImageLumaA8(la)
        }
        image::ColorType::La8 => {
            let rgba = img.to_rgba8();
            let mut la = GrayAlphaImage::new(rgba.width(), rgba.height());
            for (o, i) in la.pixels_mut().zip(rgba.pixels()) {
                o[0] = i[0];
                o[1] = alpha;
            }
            DynamicImage::ImageLumaA8(la)
        }
        image::ColorType::Rgb8 => {
            let rgb = img.to_rgb8();
            let mut rgba = RgbaImage::new(rgb.width(), rgb.height());
            for (o, i) in rgba.pixels_mut().zip(rgb.pixels()) {
                o[0] = i[0];
                o[1] = i[1];
                o[2] = i[2];
                o[3] = alpha;
            }
            DynamicImage::ImageRgba8(rgba)
        }
        _ => {
            let mut rgba = img.to_rgba8();
            for p in rgba.pixels_mut() {
                p[3] = alpha;
            }
            DynamicImage::ImageRgba8(rgba)
        }
    };
    out
}
