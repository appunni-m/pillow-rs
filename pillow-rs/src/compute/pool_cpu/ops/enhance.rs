// ── Enhance operations extracted from image.rs execute_op() ──

use crate::color::pil_grayscale;
use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::{DynamicImage, GrayAlphaImage, RgbaImage};

fn preserve_alpha_result(original: &DynamicImage, rgba: RgbaImage) -> DynamicImage {
    let (w, h) = rgba.dimensions();
    if matches!(original, DynamicImage::ImageLumaA8(_)) {
        let la = GrayAlphaImage::from_fn(w, h, |x, y| {
            let pixel = rgba.get_pixel(x, y);
            crate::raster::LumaA([pixel[0], pixel[3]])
        });
        DynamicImage::ImageLumaA8(la)
    } else {
        DynamicImage::ImageRgba8(rgba)
    }
}

pub fn op_enhance_brightness(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // CMYK mode: stored as RGBA8 (C→R, M→G, Y→B, K→A). Operate on all 4 channels.
    if mode == Some("CMYK") {
        let mut rgba = img.to_rgba8();
        let f = factor;
        for p in rgba.pixels_mut() {
            for c in 0..4 {
                p[c] = (p[c] as f64 * f).clamp(0.0, 255.0) as u8;
            }
        }
        return Ok(DynamicImage::ImageRgba8(rgba));
    }
    if matches!(
        img,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) {
        let mut rgba = img.to_rgba8();
        for pixel in rgba.pixels_mut() {
            for channel in 0..3 {
                pixel[channel] = (pixel[channel] as f64 * factor).clamp(0.0, 255.0) as u8;
            }
        }
        return Ok(preserve_alpha_result(img, rgba));
    }
    let mut rgb = img.to_rgb8();
    let f = factor;
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] = ((p[c] as f64 * f).clamp(0.0, 255.0)) as u8;
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

pub fn op_enhance_contrast(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // CMYK mode: stored as RGBA8 (C→R, M→G, Y→B, K→A)
    if mode == Some("CMYK") {
        // PIL: convert to L (via RGB), compute rounded mean, create uniform gray CMYK,
        // then blend: degenerate * (1-factor) + original * factor
        // PIL's degenerate for CMYK: C=0, M=0, Y=0, K=255-mean (NOT mean on all channels)
        let gray = pil_grayscale(img)?;
        let pixels: Vec<u8> = gray.pixels().map(|p| p[0]).collect();
        let n = pixels.len() as u64;
        let mean = if n > 0 {
            let sum: u64 = pixels.iter().map(|&p| p as u64).sum();
            ((sum as f64 / n as f64) + 0.5) as u8
        } else {
            0
        };
        let k_val = 255u8.saturating_sub(mean) as f64;
        let f = factor;
        let mut rgba = img.to_rgba8();
        for p in rgba.pixels_mut() {
            // C, M, Y: degenerate=0, so blend = 0*(1-f) + orig*f = orig*f
            // K: degenerate = 255-mean
            p[0] = (p[0] as f64 * f).clamp(0.0, 255.0) as u8;
            p[1] = (p[1] as f64 * f).clamp(0.0, 255.0) as u8;
            p[2] = (p[2] as f64 * f).clamp(0.0, 255.0) as u8;
            p[3] = (k_val * (1.0 - f) + p[3] as f64 * f).clamp(0.0, 255.0) as u8;
        }
        return Ok(DynamicImage::ImageRgba8(rgba));
    }
    // PIL: convert to L, compute rounded mean, create uniform gray degenerate,
    // then blend: degenerate * (1-factor) + original * factor
    let gray = pil_grayscale(img)?;
    let pixels: Vec<u8> = gray.pixels().map(|p| p[0]).collect();
    let n = pixels.len() as u64;
    let mean = if n > 0 {
        let sum: u64 = pixels.iter().map(|&p| p as u64).sum();
        // int(mean + 0.5) matching PIL's ImageStat
        ((sum as f64 / n as f64) + 0.5) as u8
    } else {
        0
    };
    let m = mean as f64;
    let f = factor;
    if matches!(
        img,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) {
        let mut rgba = img.to_rgba8();
        for pixel in rgba.pixels_mut() {
            for channel in 0..3 {
                pixel[channel] =
                    (m * (1.0 - f) + pixel[channel] as f64 * f).clamp(0.0, 255.0) as u8;
            }
        }
        return Ok(preserve_alpha_result(img, rgba));
    }
    let mut rgb = img.to_rgb8();
    for p in rgb.pixels_mut() {
        for c in 0..3 {
            p[c] = (m * (1.0 - f) + p[c] as f64 * f).clamp(0.0, 255.0) as u8;
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

pub fn op_enhance_color_saturation(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // CMYK mode: stored as RGBA8 (C→R, M→G, Y→B, K→A).
    // PIL Color: convert CMYK→L→CMYK to create grayscale degenerate, then blend.
    // PIL's CMYK→L→CMYK round-trip: C=0, M=0, Y=0, K=255-L (NOT (L,L,L,255-L)).
    if mode == Some("CMYK") {
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let mut out = rgba.clone();
        let f = factor;
        for y in 0..h {
            for x in 0..w {
                let p = rgba.get_pixel(x, y);
                // CMYK→RGB: R = (255-C)*(255-K)/255, G = (255-M)*(255-K)/255, B = (255-Y)*(255-K)/255
                let c = p[0] as u32;
                let m = p[1] as u32;
                let y_ = p[2] as u32;
                let k = p[3] as u32;
                let r = (255 - c) * (255 - k) / 255;
                let g = (255 - m) * (255 - k) / 255;
                let b = (255 - y_) * (255 - k) / 255;
                // BT.601 grayscale: Y = (19595*R + 38470*G + 7471*B + 32768) >> 16
                let gray_val = ((19595 * r + 38470 * g + 7471 * b + 32768) >> 16).min(255) as f64;
                // PIL degenerate for CMYK: C=0, M=0, Y=0, K=255-gray_val
                // Blend: degenerate * (1-f) + original * f
                // C = 0 * (1-f) + orig_C * f
                out.put_pixel(
                    x,
                    y,
                    crate::raster::Rgba([
                        (p[0] as f64 * f).clamp(0.0, 255.0) as u8,
                        (p[1] as f64 * f).clamp(0.0, 255.0) as u8,
                        (p[2] as f64 * f).clamp(0.0, 255.0) as u8,
                        ((255.0 - gray_val) * (1.0 - f) + p[3] as f64 * f).clamp(0.0, 255.0) as u8,
                    ]),
                );
            }
        }
        return Ok(DynamicImage::ImageRgba8(out));
    }
    // Use PIL's rounded grayscale conversion (to_luma8 truncates)
    let gray = pil_grayscale(img)?;
    if matches!(
        img,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) {
        let mut rgba = img.to_rgba8();
        let f = factor;
        for (pixel, gray_pixel) in rgba.pixels_mut().zip(gray.pixels()) {
            let g = gray_pixel[0] as f64;
            for channel in 0..3 {
                pixel[channel] = (g + f * (pixel[channel] as f64 - g)).clamp(0.0, 255.0) as u8;
            }
        }
        return Ok(preserve_alpha_result(img, rgba));
    }
    let mut rgb = img.to_rgb8();
    let f = factor;
    for (px, gp) in rgb.pixels_mut().zip(gray.pixels()) {
        let g = gp[0] as f64;
        // blend formula: gray * (1-factor) + original * factor
        px[0] = ((g + f * (px[0] as f64 - g)).clamp(0.0, 255.0)) as u8;
        px[1] = ((g + f * (px[1] as f64 - g)).clamp(0.0, 255.0)) as u8;
        px[2] = ((g + f * (px[2] as f64 - g)).clamp(0.0, 255.0)) as u8;
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

pub fn op_enhance_sharpness(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    // PIL: apply SMOOTH filter (3x3 kernel [1,1,1; 1,5,1; 1,1,1] / 13, offset 0),
    // then blend: smoothed * (1-factor) + original * factor
    let f = factor;
    // CMYK mode: operate on all 4 channels (C=R, M=G, Y=B, K=A in RGBA8)
    let has_alpha = matches!(
        img,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    );
    let channels = if mode == Some("CMYK") { 4usize } else { 3usize };
    let src = if mode == Some("CMYK") {
        img.to_rgba8().into_raw()
    } else {
        img.to_rgb8().into_raw()
    };
    let (w, h) = (img.width() as i32, img.height() as i32);
    let inv_scale = 1.0f32 / 13.0f32;
    let k = inv_scale;
    let kc = 5.0f32 * inv_scale;
    let rounding_bias = 0.5f32;
    let mut blurred = src.clone();
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            for c in 0..channels {
                let get_pixel = |dx: i32, dy: i32| -> f32 {
                    let px = (x + dx).clamp(0, w - 1) as u32;
                    let py = (y + dy).clamp(0, h - 1) as u32;
                    src[(py * w as u32 + px) as usize * channels + c] as f32
                };
                let b = get_pixel(-1, 1) * k + get_pixel(0, 1) * k + get_pixel(1, 1) * k;
                let m = get_pixel(-1, 0) * k + get_pixel(0, 0) * kc + get_pixel(1, 0) * k;
                let t = get_pixel(-1, -1) * k + get_pixel(0, -1) * k + get_pixel(1, -1) * k;
                let val = rounding_bias + b + m + t;
                let idx = (y * w + x) as usize * channels + c;
                blurred[idx] = val.clamp(0.0, 255.0) as u8;
            }
        }
    }
    let mut result = src.clone();
    for i in 0..result.len() {
        result[i] = (blurred[i] as f64 * (1.0 - f) + result[i] as f64 * f).clamp(0.0, 255.0) as u8;
    }
    if mode == Some("CMYK") {
        let img_result = crate::raster::RgbaImage::from_raw(w as u32, h as u32, result)
            .ok_or_else(|| PilError::ValueError("enhance_sharpness: buffer error".into()))?;
        Ok(DynamicImage::ImageRgba8(img_result))
    } else if has_alpha {
        let original = img.to_rgba8();
        let rgba = RgbaImage::from_fn(w as u32, h as u32, |x, y| {
            let index = (y * w as u32 + x) as usize * 3;
            let alpha = original.get_pixel(x, y)[3];
            crate::raster::Rgba([result[index], result[index + 1], result[index + 2], alpha])
        });
        Ok(preserve_alpha_result(img, rgba))
    } else {
        let img_result = crate::raster::RgbImage::from_raw(w as u32, h as u32, result)
            .ok_or_else(|| PilError::ValueError("enhance_sharpness: buffer error".into()))?;
        Ok(preserve_mode(img, DynamicImage::ImageRgb8(img_result)))
    }
}
