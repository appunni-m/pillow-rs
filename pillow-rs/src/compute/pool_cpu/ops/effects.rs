// ── Effects + Module fns + Point + Mutating operations extracted from image.rs execute_op() ──

use crate::error::PilError;
use crate::image::{preserve_mode, Image};
use crate::pipeline::{ColorMode, ResampleFilter, TransformMethod};
use pillow_rs_image::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, RgbImage, RgbaImage,
};
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
        pillow_rs_image::ColorType::L8 => {
            let luma = img.to_luma8();
            let (w, h) = luma.dimensions();
            (luma.into_raw(), w as i32, h as i32, 1usize)
        }
        pillow_rs_image::ColorType::La8 | pillow_rs_image::ColorType::La16 => {
            let la = img.to_luma_alpha8();
            let (w, h) = la.dimensions();
            (la.into_raw(), w as i32, h as i32, 2usize)
        }
        pillow_rs_image::ColorType::Rgb8 => {
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
            GrayImage::from_raw(w as u32, h as u32, out_pixels)
                .ok_or_else(|| PilError::ValueError("effect_spread buffer error".into()))?,
        ),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(w as u32, h as u32, out_pixels)
                .ok_or_else(|| PilError::ValueError("effect_spread buffer error".into()))?,
        ),
        3 => DynamicImage::ImageRgb8(
            RgbImage::from_raw(w as u32, h as u32, out_pixels)
                .ok_or_else(|| PilError::ValueError("effect_spread buffer error".into()))?,
        ),
        _ => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(w as u32, h as u32, out_pixels)
                .ok_or_else(|| PilError::ValueError("effect_spread buffer error".into()))?,
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
    let src_img = source.materialize_for_ops()?;
    let (src_w, src_h) = (src_img.width(), src_img.height());
    let paste_x = x;
    let paste_y = y;

    if let Some(mask_img_ref) = mask {
        let mask_img = mask_img_ref.materialize_for_ops()?;
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
                    let blended = pillow_rs_image::Rgba([
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
        let src_rgba = src_img.to_rgba8();
        let (sw, sh) = (src_rgba.width(), src_rgba.height());
        for py in 0..sh.min(dest_clone.height()) {
            for px in 0..sw.min(dest_clone.width()) {
                let dx = (paste_x + px as i64) as u32;
                let dy = (paste_y + py as i64) as u32;
                if dx < dest_clone.width() && dy < dest_clone.height() {
                    dest_clone.put_pixel(dx, dy, *src_rgba.get_pixel(px, py));
                }
            }
        }
        Ok(preserve_mode(img, DynamicImage::ImageRgba8(dest_clone)))
    }
}

// ── AlphaComposite ──

pub fn op_alpha_composite(
    img: &DynamicImage,
    source: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    let src_img = source.materialize_for_ops()?;
    if (src_img.width(), src_img.height()) != (img.width(), img.height()) {
        return Err(PilError::ValueError("images do not match".into()));
    }

    // LA mode: composite on native LA canvas, return LA (PIL behavior)
    if matches!(img.color(), pillow_rs_image::ColorType::La8) {
        let mut dest_la = img.to_luma_alpha8();
        let src_la = src_img.to_luma_alpha8();
        let (sw, sh) = src_la.dimensions();
        for py in 0..sh.min(dest_la.height()) {
            for px in 0..sw.min(dest_la.width()) {
                let sp = src_la.get_pixel(px, py);
                let dp = dest_la.get_pixel(px, py);
                let sa = sp[1] as f64 / 255.0;
                let da = dp[1] as f64 / 255.0;
                let out_a = sa + da * (1.0 - sa);
                if out_a <= 0.0 {
                    continue;
                }
                let l = ((sp[0] as f64 * sa + dp[0] as f64 * da * (1.0 - sa)) / out_a)
                    .round()
                    .clamp(0.0, 255.0) as u8;
                let a = (out_a * 255.0).round().clamp(0.0, 255.0) as u8;
                dest_la.put_pixel(px, py, pillow_rs_image::LumaA([l, a]));
            }
        }
        return Ok(DynamicImage::ImageLumaA8(dest_la));
    }

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
            dest_rgba.put_pixel(px, py, pillow_rs_image::Rgba([r, g, b, a]));
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
        let b_img = band.materialize_for_ops()?;
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
    let other_img = other.materialize_for_ops()?;
    if (other_img.width(), other_img.height()) != (img.width(), img.height()) {
        return Err(PilError::ValueError("images do not match".into()));
    }
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
                    pillow_rs_image::Rgba([
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
                pillow_rs_image::Rgb([
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
    // Validate size match: all three images must have the same dimensions
    let other_img_pre = other.materialize_for_ops()?;
    let mask_img_pre = mask.materialize_for_ops()?;
    if (other_img_pre.width(), other_img_pre.height()) != (img.width(), img.height())
        || (mask_img_pre.width(), mask_img_pre.height()) != (img.width(), img.height())
    {
        return Err(PilError::ValueError("images do not match".into()));
    }
    // P-mode: composite on palette indices (PIL operates on indices, not colors)
    if explicit_mode == Some("P") {
        let gray1 = img.to_luma8();
        let other_indices = other.materialize_indices()?;
        let gray2 = other_indices.to_luma8();
        let mask_img = mask.materialize_for_ops()?;
        let mask_gray = mask_img.to_luma8();
        let (w, h) = (
            gray1.width().min(gray2.width()).min(mask_gray.width()),
            gray1.height().min(gray2.height()).min(mask_gray.height()),
        );
        let mut out = GrayImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let i1 = gray1.get_pixel(x, y)[0] as u16;
                let i2 = gray2.get_pixel(x, y)[0] as u16;
                let m = mask_gray.get_pixel(x, y)[0] as u16;
                // PIL: (i1 * m + i2 * (255 - m) + 127) / 255
                let val = (i1 * m + i2 * (255 - m) + 127) / 255;
                out.put_pixel(x, y, pillow_rs_image::Luma([val as u8]));
            }
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    let other_img = other.materialize_for_ops()?;
    let mask_img = mask.materialize_for_ops()?;
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
                    pillow_rs_image::Rgba([
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
    // LA mode: composite both L and A channels natively
    if matches!(img.color(), pillow_rs_image::ColorType::La8) {
        let la1 = img.to_luma_alpha8();
        let la2 = other_img.to_luma_alpha8();
        let mask_gray = mask_img.to_luma8();
        let (w, h) = (
            la1.width().min(la2.width()).min(mask_gray.width()),
            la1.height().min(la2.height()).min(mask_gray.height()),
        );
        let mut out = GrayAlphaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let p1 = la1.get_pixel(x, y);
                let p2 = la2.get_pixel(x, y);
                let m = mask_gray.get_pixel(x, y)[0] as f64 / 255.0;
                out.put_pixel(
                    x,
                    y,
                    pillow_rs_image::LumaA([
                        ((p1[0] as f64 * m + p2[0] as f64 * (1.0 - m)).round()) as u8,
                        ((p1[1] as f64 * m + p2[1] as f64 * (1.0 - m)).round()) as u8,
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageLumaA8(out));
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
                pillow_rs_image::Rgb([
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
        pillow_rs_image::ColorType::L8 | pillow_rs_image::ColorType::L16 => 1,
        pillow_rs_image::ColorType::La8 | pillow_rs_image::ColorType::La16 => 2,
        pillow_rs_image::ColorType::Rgb8 | pillow_rs_image::ColorType::Rgb16 => 3,
        _ => 4,
    };
    // PIL requires EXACTLY 256 * n_bands lut entries
    let expected = 256 * n_bands;
    if lut.len() != expected {
        return Err(PilError::ValueError("wrong number of lut entries".into()));
    }
    let band_luts: Vec<&[u8]> = (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect();
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
    // Use glibc-compatible PRNG with seed 1 (PIL's default rand() seed)
    let mut rng = GlibcRand::new(1);
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
        pillow_rs_image::ColorType::L8 | pillow_rs_image::ColorType::L16 => 1,
        pillow_rs_image::ColorType::La8 | pillow_rs_image::ColorType::La16 => 2,
        pillow_rs_image::ColorType::Rgb8 | pillow_rs_image::ColorType::Rgb16 => 3,
        _ => 4,
    };
    // PIL requires EXACTLY 256 * n_bands lut entries
    let expected = 256 * n_bands;
    if lut.len() != expected {
        return Err(PilError::ValueError("wrong number of lut entries".into()));
    }
    let band_luts: Vec<&[u8]> = (0..n_bands).map(|b| &lut[b * 256..(b + 1) * 256]).collect();
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
        &TransformMethod::Mesh => {
            if data.len() < 12 {
                return Err(PilError::ValueError(
                    "Mesh transform needs at least 12 values per element".into(),
                ));
            }
            let result = transform_mesh(img, w, h, data, fill);
            Ok(preserve_mode(img, result?))
        }
        &TransformMethod::Perspective | &TransformMethod::Quad => Err(
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
    let (w, h) = (img.width(), img.height());
    if x >= w || y >= h {
        return Err(PilError::IndexError("image index out of range".into()));
    }
    match img.clone() {
        DynamicImage::ImageLuma8(mut l) => {
            l.put_pixel(x, y, pillow_rs_image::Luma([color.0]));
            Ok(DynamicImage::ImageLuma8(l))
        }
        DynamicImage::ImageLumaA8(mut la) => {
            la.put_pixel(x, y, pillow_rs_image::LumaA([color.0, color.3]));
            Ok(DynamicImage::ImageLumaA8(la))
        }
        DynamicImage::ImageRgb8(mut rgb) => {
            rgb.put_pixel(x, y, pillow_rs_image::Rgb([color.0, color.1, color.2]));
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        DynamicImage::ImageRgba8(mut rgba) => {
            rgba.put_pixel(
                x,
                y,
                pillow_rs_image::Rgba([color.0, color.1, color.2, color.3]),
            );
            Ok(DynamicImage::ImageRgba8(rgba))
        }
        _ => Err(PilError::NotImplementedError(
            "putpixel not supported for this image type".into(),
        )),
    }
}

// ── PutData ──

pub fn op_put_data(
    img: &DynamicImage,
    data: &[u8],
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let expected = match img.color() {
        pillow_rs_image::ColorType::L8 => w * h,
        pillow_rs_image::ColorType::La8 => w * h * 2,
        pillow_rs_image::ColorType::Rgb8 => w * h * 3,
        _ => w * h * 4,
    };
    // PIL: putdata accepts data shorter than the image — only the first
    // data.len() bytes are replaced; remaining pixels stay unchanged.
    let n_copy = data.len().min(expected);
    let clip = explicit_mode == Some("1");
    match img.color() {
        pillow_rs_image::ColorType::Rgb8 => {
            let orig = img.to_rgb8();
            let mut pixels = orig.into_raw();
            for (i, &v) in data[..n_copy].iter().enumerate() {
                pixels[i] = if clip && v != 0 { 255 } else { v };
            }
            let rgb = RgbImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageRgb8(rgb))
        }
        pillow_rs_image::ColorType::L8 => {
            let orig = img.to_luma8();
            let mut pixels = orig.into_raw();
            for (i, &v) in data[..n_copy].iter().enumerate() {
                pixels[i] = if clip && v != 0 { 255 } else { v };
            }
            let gray = GrayImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageLuma8(gray))
        }
        pillow_rs_image::ColorType::La8 => {
            let orig = img.to_luma_alpha8();
            let mut pixels = orig.into_raw();
            pixels[..n_copy].copy_from_slice(&data[..n_copy]);
            let la = GrayAlphaImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageLumaA8(la))
        }
        _ => {
            let orig = img.to_rgba8();
            let mut pixels = orig.into_raw();
            pixels[..n_copy].copy_from_slice(&data[..n_copy]);
            let rgba = RgbaImage::from_raw(w as u32, h as u32, pixels)
                .ok_or_else(|| PilError::ValueError("putdata: buffer error".into()))?;
            Ok(DynamicImage::ImageRgba8(rgba))
        }
    }
}

// ── PutAlpha ──

pub fn op_put_alpha(img: &DynamicImage, alpha: u8, explicit_mode: Option<&str>) -> DynamicImage {
    // Handle explicit PIL modes that need special treatment
    if let Some(mode) = explicit_mode {
        match mode {
            "CMYK" => {
                // PIL putalpha on CMYK converts to RGBA (proper color space),
                // sets alpha, and returns RGBA.
                // CMYK→RGB: R = 255*(1-C/255)*(1-K/255), etc.
                let raw = img.as_bytes();
                let (w, h) = img.dimensions();
                let mut rgba = RgbaImage::new(w, h);
                for (i, p) in rgba.pixels_mut().enumerate() {
                    let c = raw[i * 4] as f64 / 255.0;
                    let m = raw[i * 4 + 1] as f64 / 255.0;
                    let y = raw[i * 4 + 2] as f64 / 255.0;
                    let k = raw[i * 4 + 3] as f64 / 255.0;
                    p[0] = (255.0 * (1.0 - c) * (1.0 - k) + 0.5) as u8;
                    p[1] = (255.0 * (1.0 - m) * (1.0 - k) + 0.5) as u8;
                    p[2] = (255.0 * (1.0 - y) * (1.0 - k) + 0.5) as u8;
                    p[3] = alpha;
                }
                return DynamicImage::ImageRgba8(rgba);
            }
            "P" => {
                // PIL putalpha on P converts to PA (palette index + alpha).
                // Stored as Luma8, convert to LumaA8 with alpha.
                let luma = img.to_luma8();
                let mut la = GrayAlphaImage::new(luma.width(), luma.height());
                for (o, i) in la.pixels_mut().zip(luma.pixels()) {
                    o[0] = i[0];
                    o[1] = alpha;
                }
                return DynamicImage::ImageLumaA8(la);
            }
            _ => {}
        }
    }
    let out = match img.color() {
        pillow_rs_image::ColorType::L8 => {
            let luma = img.to_luma8();
            let mut la = GrayAlphaImage::new(luma.width(), luma.height());
            for (o, i) in la.pixels_mut().zip(luma.pixels()) {
                o[0] = i[0];
                o[1] = alpha;
            }
            DynamicImage::ImageLumaA8(la)
        }
        pillow_rs_image::ColorType::La8 => {
            let rgba = img.to_rgba8();
            let mut la = GrayAlphaImage::new(rgba.width(), rgba.height());
            for (o, i) in la.pixels_mut().zip(rgba.pixels()) {
                o[0] = i[0];
                o[1] = alpha;
            }
            DynamicImage::ImageLumaA8(la)
        }
        pillow_rs_image::ColorType::Rgb8 => {
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

// ── Color3DLUT — trilinear interpolation (matching PIL's _imaging C code) ──

fn table_index_3d(x: usize, y: usize, z: usize, sx: usize, sxy: usize) -> usize {
    x + y * sx + z * sxy
}

pub fn op_color3dlut(
    img: &DynamicImage,
    size: (u32, u32, u32),
    table: &[f64],
    channels: u32,
) -> Result<DynamicImage, PilError> {
    let (sx, sy, sz) = (size.0 as usize, size.1 as usize, size.2 as usize);
    let ch = channels as usize;
    let sxy = sx * sy;

    let (w, h) = img.dimensions();
    let src_channels = img.color().channel_count() as usize;

    // Precompute grid mapping: pixel value → fractional grid coordinate
    let scale_x = (sx - 1) as f64 / 255.0;
    let scale_y = (sy - 1) as f64 / 255.0;
    let scale_z = (sz - 1) as f64 / 255.0;

    let mut out = vec![0u8; (w * h) as usize * 4];

    for y in 0..h {
        for x in 0..w {
            let out_idx = ((y * w + x) as usize) * 4;
            let px = img.get_pixel(x, y).0;

            let fx = px[0] as f64 * scale_x;
            let fy = px[1] as f64 * scale_y;
            let fz = px[2] as f64 * scale_z;

            let x0 = (fx.floor() as usize).min(sx - 1);
            let y0 = (fy.floor() as usize).min(sy - 1);
            let z0 = (fz.floor() as usize).min(sz - 1);
            let x1 = (x0 + 1).min(sx - 1);
            let y1 = (y0 + 1).min(sy - 1);
            let z1 = (z0 + 1).min(sz - 1);

            let dx = fx - x0 as f64;
            let dy = fy - y0 as f64;
            let dz = fz - z0 as f64;

            let w000 = (1.0 - dx) * (1.0 - dy) * (1.0 - dz);
            let w100 = dx * (1.0 - dy) * (1.0 - dz);
            let w010 = (1.0 - dx) * dy * (1.0 - dz);
            let w110 = dx * dy * (1.0 - dz);
            let w001 = (1.0 - dx) * (1.0 - dy) * dz;
            let w101 = dx * (1.0 - dy) * dz;
            let w011 = (1.0 - dx) * dy * dz;
            let w111 = dx * dy * dz;

            let base000 = table_index_3d(x0, y0, z0, sx, sxy) * ch;
            let base100 = table_index_3d(x1, y0, z0, sx, sxy) * ch;
            let base010 = table_index_3d(x0, y1, z0, sx, sxy) * ch;
            let base110 = table_index_3d(x1, y1, z0, sx, sxy) * ch;
            let base001 = table_index_3d(x0, y0, z1, sx, sxy) * ch;
            let base101 = table_index_3d(x1, y0, z1, sx, sxy) * ch;
            let base011 = table_index_3d(x0, y1, z1, sx, sxy) * ch;
            let base111 = table_index_3d(x1, y1, z1, sx, sxy) * ch;

            for c in 0..ch {
                let v = w000 * table[base000 + c]
                    + w100 * table[base100 + c]
                    + w010 * table[base010 + c]
                    + w110 * table[base110 + c]
                    + w001 * table[base001 + c]
                    + w101 * table[base101 + c]
                    + w011 * table[base011 + c]
                    + w111 * table[base111 + c];
                // PIL uses _prepare_lut_table which does item * 16320 + 0.5 (round to INT16),
                // then clip8 does (result + 32) >> 6 = (result / 64) truncated with rounding.
                // Equivalent to: round(v * 255.0) clamped to [0, 255]
                let clipped = (v * 255.0 + 0.5).floor().max(0.0).min(255.0) as u8;
                out[out_idx + c] = clipped;
            }
            if ch == 3 {
                out[out_idx + 3] = if src_channels >= 4 { px[3] } else { 255 };
            }
        }
    }

    // Preserve input color type (RGB input → RGB output, RGBA → RGBA)
    let result = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(w, h, out).expect("color3dlut: buffer size mismatch"),
    );
    Ok(preserve_mode(img, result))
}

// ── MESH transform — piecewise bilinear quad mapping ──

pub fn transform_mesh(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    mesh_data: &[f64],
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let sw_f = sw as f64;
    let sh_f = sh as f64;
    let fill_color = fill.unwrap_or((0, 0, 0, 255));

    let mut out = vec![0u8; (dst_w * dst_h) as usize * 4];
    // Initialize output with fill color
    for y in 0..dst_h as usize {
        for x in 0..dst_w as usize {
            let idx = (y * dst_w as usize + x) * 4;
            out[idx] = fill_color.0;
            out[idx + 1] = fill_color.1;
            out[idx + 2] = fill_color.2;
            out[idx + 3] = fill_color.3;
        }
    }

    // Process each mesh element
    let num_elements = mesh_data.len() / 12;
    for elem in 0..num_elements {
        let base = elem * 12;
        let x0_d = mesh_data[base] as i32;
        let y0_d = mesh_data[base + 1] as i32;
        let x1_d = mesh_data[base + 2] as i32;
        let y1_d = mesh_data[base + 3] as i32;
        let x0_s = mesh_data[base + 4];
        let y0_s = mesh_data[base + 5];
        let x1_s = mesh_data[base + 6];
        let y1_s = mesh_data[base + 7];
        let x2_s = mesh_data[base + 8];
        let y2_s = mesh_data[base + 9];
        let x3_s = mesh_data[base + 10];
        let y3_s = mesh_data[base + 11];

        let bw = (x1_d - x0_d) as f64;
        let bh = (y1_d - y0_d) as f64;
        if bw <= 0.0 || bh <= 0.0 {
            continue;
        }

        let bx0 = x0_d.max(0);
        let by0 = y0_d.max(0);
        let bx1 = x1_d.min(dst_w as i32);
        let by1 = y1_d.min(dst_h as i32);

        for dy in by0..by1 {
            let v = (dy - y0_d) as f64 / bh;
            for dx in bx0..bx1 {
                let u = (dx - x0_d) as f64 / bw;

                // PIL bilinear mapping: quad[0]=top-left, quad[1]=bottom-left,
                // quad[2]=bottom-right, quad[3]=top-right (counter-clockwise)
                let sx = (1.0 - u) * (1.0 - v) * x0_s
                    + u * (1.0 - v) * x3_s
                    + u * v * x2_s
                    + (1.0 - u) * v * x1_s;
                let sy = (1.0 - u) * (1.0 - v) * y0_s
                    + u * (1.0 - v) * y3_s
                    + u * v * y2_s
                    + (1.0 - u) * v * y1_s;

                if sx >= 0.0 && sx < sw_f && sy >= 0.0 && sy < sh_f {
                    // NEAREST sampling for identity mesh parity
                    let ix = (sx + 0.5).floor() as u32;
                    let iy = (sy + 0.5).floor() as u32;
                    let src_idx = ((iy * sw + ix) as usize) * channels;
                    let out_idx = ((dy as u32 * dst_w + dx as u32) as usize) * 4;
                    for c in 0..channels {
                        out[out_idx + c] = raw[src_idx + c];
                    }
                    if channels < 4 {
                        out[out_idx + 3] = 255;
                    }
                }
            }
        }
    }

    Ok(DynamicImage::ImageRgba8(
        RgbaImage::from_raw(dst_w, dst_h, out).expect("transform_mesh: buffer size mismatch"),
    ))
}

/// Generate a Mandelbrot set fractal image.
/// For each pixel (px, py) in w×h, maps to complex plane via extent [x0,y0]→[x1,y1],
/// iterates z = z² + c up to `quality` times, outputs iteration count as grayscale.
pub fn op_effect_mandelbrot(
    w: u32,
    h: u32,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    quality: u32,
) -> Result<DynamicImage, PilError> {
    let mut gray = pillow_rs_image::GrayImage::new(w, h);
    let dx = (x1 - x0) / w as f64;
    let dy = (y1 - y0) / h as f64;
    for py in 0..h {
        let cy = y0 + py as f64 * dy;
        for px in 0..w {
            let cx = x0 + px as f64 * dx;
            let (mut zx, mut zy) = (0.0f64, 0.0f64);
            let mut iter: u32 = 0;
            while iter < quality {
                let zx2 = zx * zx - zy * zy + cx;
                let zy2 = 2.0 * zx * zy + cy;
                zx = zx2;
                zy = zy2;
                if zx * zx + zy * zy > 4.0 {
                    break;
                }
                iter += 1;
            }
            let val = (iter * 255 / quality.max(1)) as u8;
            gray.put_pixel(px, py, pillow_rs_image::Luma([val]));
        }
    }
    Ok(DynamicImage::ImageLuma8(gray))
}
