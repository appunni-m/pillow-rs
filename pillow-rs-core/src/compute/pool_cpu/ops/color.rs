//! Color/Convert CPU operations extracted from image.rs execute_op().
//! These implement PIL-compatible color mode conversion, quantization, and palette remapping.

use image::DynamicImage;

use crate::color::{pil_grayscale, pil_grayscale_truncate};
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::ops::quantize::median_cut_quantize_rgb;
use crate::pipeline::{ColorMode, DitherMethod};

/// Convert image to a specified color mode.
/// Matches PIL's Image.convert() behavior exactly.
/// `explicit_mode` is the PIL mode string on the source image.
/// `palette` is the palette data for P-mode images.
pub fn op_convert(
    img: &DynamicImage,
    mode: &ColorMode,
    dither: Option<&DitherMethod>,
    explicit_mode: Option<&str>,
    palette: Option<&[u8]>,
) -> Result<DynamicImage, PilError> {
    match mode {
        ColorMode::L => Ok(DynamicImage::ImageLuma8(pil_grayscale(img))),
        ColorMode::LA => {
            let gray = pil_grayscale(img);
            let (w, h) = gray.dimensions();
            let mut ga = image::GrayAlphaImage::new(w, h);
            for (gap, gp) in ga.pixels_mut().zip(gray.pixels()) {
                gap[0] = gp[0];
                gap[1] = 255;
            }
            Ok(DynamicImage::ImageLumaA8(ga))
        }
        ColorMode::RGB => {
            // P-mode images store palette indices in Luma8. When converting
            // to RGB, expand indices through the palette to get actual colors.
            if explicit_mode == Some("P") {
                if let Some(pal) = palette {
                    let gray = img.to_luma8();
                    let (w, h) = gray.dimensions();
                    let mut out = image::RgbImage::new(w, h);
                    for (opx, ip) in out.pixels_mut().zip(gray.pixels()) {
                        let idx = ip[0] as usize * 3;
                        opx[0] = pal.get(idx).copied().unwrap_or(0);
                        opx[1] = pal.get(idx + 1).copied().unwrap_or(0);
                        opx[2] = pal.get(idx + 2).copied().unwrap_or(0);
                    }
                    return Ok(DynamicImage::ImageRgb8(out));
                }
            }
            Ok(DynamicImage::ImageRgb8(img.to_rgb8()))
        }
        ColorMode::RGBA => Ok(DynamicImage::ImageRgba8(img.to_rgba8())),
        ColorMode::Mode1 => {
            // PIL uses TRUNCATED grayscale for convert("1") (dither or no dither)
            // while convert("L") uses ROUNDED grayscale.
            let gray = pil_grayscale_truncate(img);
            let (w, h) = gray.dimensions();
            let mut out = image::GrayImage::new(w, h);
            match dither {
                Some(DitherMethod::None) => {
                    // Threshold at 128 (no dither)
                    for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                        op[0] = if gp[0] >= 128 { 255 } else { 0 };
                    }
                }
                _ => {
                    // PIL-compatible Floyd-Steinberg dither using PIL's scaled-error pattern.
                    // Single errors array [w+1]; running l0/l1 carry error between rows.
                    // Truncation-toward-zero division, no intermediate clipping.
                    let mut errors = vec![0i32; (w + 1) as usize];
                    let src: Vec<i32> = gray.pixels().map(|p| p[0] as i32).collect();
                    let mut fs_out = vec![0u8; (w * h) as usize];
                    let wu = w as usize;
                    for y in 0..h as usize {
                        let mut l = 0i32;
                        let mut l0: i32 = 0;
                        let mut l1: i32 = 0;
                        for x in 0..wu {
                            let idx = y * wu + x;
                            let acc = l + errors[x + 1];
                            let v = src[idx] + acc / 16;
                            let v = v.clamp(0, 255);
                            let new = if v > 128 { 255i32 } else { 0i32 };
                            fs_out[idx] = new as u8;
                            l = v - new;
                            let l2 = l;
                            let d2 = l + l;
                            l += d2;
                            errors[x] = l + l0;
                            l += d2;
                            l0 = l + l1;
                            l1 = l2;
                            l += d2;
                        }
                    }
                    for (op, &gp) in out.pixels_mut().zip(fs_out.iter()) {
                        op[0] = gp;
                    }
                }
            }
            Ok(DynamicImage::ImageLuma8(out))
        }
        ColorMode::P => {
            // convert("P") = quantize(256) with dither
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let n = (w * h) as usize;
            let rgb_raw = rgb.into_raw();
            let (indices, _palette) = median_cut_quantize_rgb(&rgb_raw, 256);
            let mut out = image::GrayImage::new(w, h);
            for (i, pixel) in out.pixels_mut().enumerate().take(n) {
                pixel[0] = indices.get(i).copied().unwrap_or(0);
            }
            Ok(DynamicImage::ImageLuma8(out))
        }
        ColorMode::I => {
            // Convert to int32 mode: grayscale values stored as RGBA (int32 LE)
            let gray = pil_grayscale(img);
            let (w, h) = gray.dimensions();
            let mut out = image::RgbaImage::new(w, h);
            for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                let val = gp[0] as i32;
                let le = val.to_le_bytes();
                *op = image::Rgba([le[0], le[1], le[2], le[3]]);
            }
            Ok(DynamicImage::ImageRgba8(out))
        }
        ColorMode::F => {
            // Convert to float32 mode: grayscale values stored as RGBA (f32 LE)
            let gray = pil_grayscale(img);
            let (w, h) = gray.dimensions();
            let mut out = image::RgbaImage::new(w, h);
            for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                let val = gp[0] as f32;
                let le = val.to_le_bytes();
                *op = image::Rgba([le[0], le[1], le[2], le[3]]);
            }
            Ok(DynamicImage::ImageRgba8(out))
        }
        ColorMode::CMYK => {
            // Convert to CMYK: RGB → CMYK conversion (PIL inversion formula)
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = image::RgbaImage::new(w, h);
            for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
                let r = ip[0] as f64 / 255.0;
                let g = ip[1] as f64 / 255.0;
                let b = ip[2] as f64 / 255.0;
                let k = 1.0 - r.max(g.max(b));
                let c = if k < 1.0 {
                    (1.0 - r - k) / (1.0 - k)
                } else {
                    0.0
                };
                let m = if k < 1.0 {
                    (1.0 - g - k) / (1.0 - k)
                } else {
                    0.0
                };
                let y = if k < 1.0 {
                    (1.0 - b - k) / (1.0 - k)
                } else {
                    0.0
                };
                *op = image::Rgba([
                    (c * 255.0 + 0.5) as u8,
                    (m * 255.0 + 0.5) as u8,
                    (y * 255.0 + 0.5) as u8,
                    (k * 255.0 + 0.5) as u8,
                ]);
            }
            Ok(DynamicImage::ImageRgba8(out))
        }
        _ => Err(PilError::NotImplementedError(format!(
            "Convert to {:?} not yet implemented",
            mode
        ))),
    }
}

/// Quantize image to a palette of `colors` entries.
/// Uses median-cut quantization, matching PIL's behavior.
pub fn op_quantize(
    img: &DynamicImage,
    colors: usize,
    _dither: Option<&DitherMethod>,
) -> Result<DynamicImage, PilError> {
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let n = (w * h) as usize;
    if n == 0 {
        return Err(PilError::ValueError("quantize: empty image".into()));
    }
    let colors = colors.clamp(2, 256);
    let rgb_raw = rgb.into_raw();
    if rgb_raw.len() < colors * 3 {
        return Err(PilError::ValueError(
            "quantize: not enough pixel data".into(),
        ));
    }
    // Use median-cut quantization instead of NeuQuant.
    let (indices, _palette) = median_cut_quantize_rgb(&rgb_raw, colors);
    let mut out = image::GrayImage::new(w, h);
    for (i, pixel) in out.pixels_mut().enumerate().take(n) {
        pixel[0] = indices.get(i).copied().unwrap_or(0);
    }
    Ok(DynamicImage::ImageLuma8(out))
}

/// Remap palette indices according to a destination map.
/// PIL builds inverse lookup: inverse[dest_map[i]] = i, all else -> 0
pub fn op_remap_palette(
    img: &DynamicImage,
    dest_map: &[u8; 256],
    explicit_mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // PIL builds inverse lookup: inverse[dest_map[i]] = i, all else -> 0
    let mut inverse = [0u8; 256];
    for (i, &old_pos) in dest_map.iter().enumerate() {
        let old_idx = old_pos as usize;
        if old_idx < 256 {
            inverse[old_idx] = i as u8;
        }
    }
    // P-mode: operate on palette indices directly.
    if explicit_mode == Some("P") {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = image::GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            op[0] = inverse[ip[0] as usize];
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    // L-mode: operate on each luma value, returning P-mode output
    if img.color() == image::ColorType::L8 {
        let gray = img.to_luma8();
        let (w, h) = gray.dimensions();
        let mut out = image::GrayImage::new(w, h);
        for (op, ip) in out.pixels_mut().zip(gray.pixels()) {
            op[0] = inverse[ip[0] as usize];
        }
        return Ok(DynamicImage::ImageLuma8(out));
    }
    // Non-P, non-L: operate on each RGB channel.
    let rgb = img.to_rgb8();
    let (w, h) = rgb.dimensions();
    let mut out = image::RgbImage::new(w, h);
    for (op, ip) in out.pixels_mut().zip(rgb.pixels()) {
        op[0] = inverse[ip[0] as usize];
        op[1] = inverse[ip[1] as usize];
        op[2] = inverse[ip[2] as usize];
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}
