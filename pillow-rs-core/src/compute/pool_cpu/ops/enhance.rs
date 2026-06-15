// ── Enhance operations extracted from image.rs execute_op() ──

use crate::color::pil_grayscale;
use crate::error::PilError;
use crate::image::preserve_mode;
use image::{DynamicImage, GrayImage, GrayAlphaImage, RgbImage, RgbaImage};

/// Check if a mode string has an alpha channel (LA, RGBA).
fn mode_has_alpha(mode: Option<&str>) -> bool {
    matches!(mode, None | Some("LA") | Some("RGBA"))
}

/// Convert a mode string to a dynamic image for the degenerate (solid color).
/// For CMYK: solid (0,0,0,0) for brightness, or solid gray via L→CMYK conversion.
fn make_solid_dynamic(w: u32, h: u32, channels: usize, pixel: &[u8]) -> DynamicImage {
    match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_pixel(w, h, image::Luma([pixel[0]]))),
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_pixel(w, h, image::LumaA([pixel[0], pixel[1]]))),
        3 => DynamicImage::ImageRgb8(RgbImage::from_pixel(w, h, image::Rgb([pixel[0], pixel[1], pixel[2]]))),
        _ => DynamicImage::ImageRgba8(RgbaImage::from_pixel(w, h, image::Rgba([pixel[0], pixel[1], pixel[2], pixel[3]]))),
    }
}

/// Blend two images: out = degen * (1-factor) + orig * factor (per-channel).
/// This matches PIL's Image.blend behavior.
fn blend_images(orig: &DynamicImage, degen: &DynamicImage, factor: f64, mode: Option<&str>) -> DynamicImage {
    let channels = orig.color().channel_count() as usize;
    let has_alpha = mode_has_alpha(mode);
    let w = orig.width();
    let h = orig.height();
    let orig_bytes = orig.as_bytes();
    let degen_bytes = degen.as_bytes();
    let out_channels = if has_alpha { channels } else { channels };
    let mut out = vec![0u8; (w * h) as usize * out_channels];
    let stride = w as usize * out_channels;

    for y in 0..h as usize {
        for x in 0..w as usize {
            for c in 0..channels {
                let idx = y * stride + x * channels + c;
                // For modes with alpha, preserve the alpha channel
                if has_alpha && c == channels - 1 {
                    out[idx] = orig_bytes[idx];
                } else {
                    let d_val = degen_bytes.get(idx).copied().unwrap_or(0) as f64;
                    let o_val = orig_bytes[idx] as f64;
                    let v = d_val * (1.0 - factor) + o_val * factor;
                    out[idx] = v.clamp(0.0, 255.0) as u8;
                }
            }
        }
    }

    let result = match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(w, h, out).expect("blend L buffer")),
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(w, h, out).expect("blend LA buffer")),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, out).expect("blend RGB buffer")),
        _ => DynamicImage::ImageRgba8(RgbaImage::from_raw(w, h, out).expect("blend RGBA buffer")),
    };
    preserve_mode(orig, result)
}

/// Convert a grayscale value to CMYK degenerate pixel: C=0, M=0, Y=0, K=255-L
fn gray_to_cmyk_degenerate(gray_val: u8) -> [u8; 4] {
    let k = 255u8 - gray_val;
    [0, 0, 0, k]
}

/// Compute PIL-compatible CMYK→L conversion.
/// For CMYK stored as Rgba8: performs proper subtractive→additive→luminance conversion.
fn cmyk_to_l(img: &DynamicImage) -> GrayImage {
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let mut gray = GrayImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let p = rgba.get_pixel(x, y);
            let c = p[0] as u32;
            let m = p[1] as u32;
            let y_ = p[2] as u32;
            let k = p[3] as u32;
            // CMYK→RGB: R = (255-C)*(255-K)/255, G = (255-M)*(255-K)/255, B = (255-Y)*(255-K)/255
            let r = (255 - c) * (255 - k) / 255;
            let g = (255 - m) * (255 - k) / 255;
            let b = (255 - y_) * (255 - k) / 255;
            // BT.601 luminance: (19595*R + 38470*G + 7471*B + 32768) >> 16
            let l = (19595 * r + 38470 * g + 7471 * b + 32768) >> 16;
            gray.put_pixel(x, y, image::Luma([l.min(255) as u8]));
        }
    }
    gray
}

pub fn op_enhance_brightness(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let has_alpha = mode_has_alpha(mode);
    let f = factor;
    // Create degenerate: all channels 0
    let w = img.width();
    let h = img.height();
    let degen_pixel: Vec<u8> = (0..channels).map(|_| 0u8).collect();
    let degen = make_solid_dynamic(w, h, channels, &degen_pixel);
    // For modes with alpha, copy alpha from original
    let degen = if has_alpha {
        let orig_bytes = img.as_bytes();
        let degen_bytes = degen.as_bytes();
        let mut degen_data = degen_bytes.to_vec();
        let stride = w as usize * channels;
        for y in 0..h as usize {
            for x in 0..w as usize {
                let idx = y * stride + x * channels + (channels - 1);
                degen_data[idx] = orig_bytes[idx];
            }
        }
        match channels {
            2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(w, h, degen_data).expect("brightness LA")),
            _ => DynamicImage::ImageRgba8(RgbaImage::from_raw(w, h, degen_data).expect("brightness RGBA")),
        }
    } else {
        degen
    };
    // Blend: degen * (1-f) + orig * f  = orig * f (since degen=0 for non-alpha)
    // For modes with alpha: alpha is preserved via blend_images
    Ok(blend_images(img, &degen, f, mode))
}

pub fn op_enhance_contrast(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let f = factor;
    let w = img.width();
    let h = img.height();
    let channels = img.color().channel_count() as usize;
    let is_cmyk = mode == Some("CMYK");

    // Compute mean luminance
    let gray = if is_cmyk {
        cmyk_to_l(img)
    } else {
        pil_grayscale(img)
    };
    let pixels: Vec<u8> = gray.pixels().map(|p| p[0]).collect();
    let n = pixels.len() as u64;
    let mean = if n > 0 {
        let sum: u64 = pixels.iter().map(|&p| p as u64).sum();
        // int(mean + 0.5) matching PIL's ImageStat
        ((sum as f64 / n as f64) + 0.5) as u8
    } else {
        0
    };

    // Create degenerate: solid gray in the image's mode
    let degen_pixel: Vec<u8> = if is_cmyk {
        gray_to_cmyk_degenerate(mean).to_vec()
    } else {
        // For L/LA/RGB/RGBA: solid gray = mean for each channel
        let rep = if channels >= 3 { 3 } else { channels };
        let mut pixel = vec![mean; channels];
        if channels == 4 {
            // For RGBA, set R=G=B=mean, preserve alpha
            pixel[3] = 0; // alpha will be filled from original
        }
        pixel
    };
    let degen = make_solid_dynamic(w, h, channels, &degen_pixel);

    // For modes with alpha, copy alpha from original
    let has_alpha = mode_has_alpha(mode);
    let degen = if has_alpha {
        let orig_bytes = img.as_bytes();
        let degen_bytes = degen.as_bytes();
        let mut degen_data = degen_bytes.to_vec();
        let stride = w as usize * channels;
        for y in 0..h as usize {
            for x in 0..w as usize {
                let idx = y * stride + x * channels + (channels - 1);
                degen_data[idx] = orig_bytes[idx];
            }
        }
        match channels {
            2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(w, h, degen_data).expect("contrast LA")),
            _ => DynamicImage::ImageRgba8(RgbaImage::from_raw(w, h, degen_data).expect("contrast RGBA")),
        }
    } else {
        degen
    };

    Ok(blend_images(img, &degen, f, mode))
}

pub fn op_enhance_color_saturation(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let f = factor;
    let w = img.width();
    let h = img.height();
    let channels = img.color().channel_count() as usize;
    let is_cmyk = mode == Some("CMYK");
    let has_alpha = mode_has_alpha(mode);

    // Create degenerate: grayscale version converted to image's mode
    let gray = if is_cmyk {
        cmyk_to_l(img)
    } else {
        pil_grayscale(img)
    };

    // Build degenerate pixel-by-pixel matching PIL's
    // image.convert("L").convert(image.mode)
    let orig_bytes = img.as_bytes();
    let stride = w as usize * channels;
    let mut degen_data = vec![0u8; (w * h) as usize * channels];

    for y in 0..h as usize {
        for x in 0..w as usize {
            let g = gray.get_pixel(x as u32, y as u32)[0];
            for c in 0..channels {
                let idx = y * stride + x * channels + c;
                if is_cmyk {
                    // L→CMYK: C=M=Y=0, K=255-L
                    let vals = gray_to_cmyk_degenerate(g);
                    if c == 3 {
                        degen_data[idx] = vals[3];
                    } else {
                        degen_data[idx] = 0;
                    }
                } else if has_alpha && c == channels - 1 {
                    // Preserve alpha in degenerate
                    degen_data[idx] = orig_bytes[idx];
                } else {
                    // For L/LA/RGB/RGBA: set all non-alpha channels to gray value
                    degen_data[idx] = g;
                }
            }
        }
    }

    let degen = match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(w, h, degen_data).expect("color L")),
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(w, h, degen_data).expect("color LA")),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, degen_data).expect("color RGB")),
        _ => DynamicImage::ImageRgba8(RgbaImage::from_raw(w, h, degen_data).expect("color RGBA")),
    };

    Ok(blend_images(img, &degen, f, mode))
}

pub fn op_enhance_sharpness(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let f = factor;
    let channels = img.color().channel_count() as usize;
    let is_cmyk = mode == Some("CMYK");
    let has_alpha = mode_has_alpha(mode);
    let (w, h) = (img.width() as i32, img.height() as i32);

    // Get image data
    let rgba = img.to_rgba8();
    let (ww, wh) = (rgba.width() as i32, rgba.height() as i32);

    // Apply SMOOTH filter (3x3 kernel [1,1,1; 1,5,1; 1,1,1] / 13, offset 0)
    // on all non-alpha channels
    let data_channels = if is_cmyk { 4 } else { 3.min(channels) };
    let n_data = if is_cmyk { 4 } else if has_alpha { channels - 1 } else { channels };

    let mut blurred = rgba.clone();
    let inv_scale = 1.0f32 / 13.0f32;
    let k = inv_scale;
    let kc = 5.0f32 * inv_scale;
    let rounding_bias = 0.5f32;

    for y in 1..wh - 1 {
        for x in 1..ww - 1 {
            let bp = rgba.get_pixel((x - 1) as u32, (y + 1) as u32);
            let cp = rgba.get_pixel(x as u32, (y + 1) as u32);
            let ap = rgba.get_pixel((x + 1) as u32, (y + 1) as u32);
            let bp_c = rgba.get_pixel((x - 1) as u32, y as u32);
            let cp_c = rgba.get_pixel(x as u32, y as u32);
            let ap_c = rgba.get_pixel((x + 1) as u32, y as u32);
            let bp_t = rgba.get_pixel((x - 1) as u32, (y - 1) as u32);
            let cp_t = rgba.get_pixel(x as u32, (y - 1) as u32);
            let ap_t = rgba.get_pixel((x + 1) as u32, (y - 1) as u32);

            let out_p = rgba.get_pixel(x as u32, y as u32);
            let mut new_p = *out_p;
            for c in 0..4 {
                // Skip alpha channel for non-CMYK RGBA
                if c >= n_data {
                    continue;
                }
                let row_b = bp[c] as f32 * k + cp[c] as f32 * k + ap[c] as f32 * k;
                let row_c = bp_c[c] as f32 * k + cp_c[c] as f32 * kc + ap_c[c] as f32 * k;
                let row_t = bp_t[c] as f32 * k + cp_t[c] as f32 * k + ap_t[c] as f32 * k;
                let mut acc = rounding_bias;
                acc += row_b + row_c + row_t;
                new_p[c] = acc.clamp(0.0, 255.0) as u8;
            }
            blurred.put_pixel(x as u32, y as u32, new_p);
        }
    }

    // Blend: blurred * (1-f) + original * f
    let mut result = rgba.clone();
    for y in 0..wh {
        for x in 0..ww {
            let op = rgba.get_pixel(x as u32, y as u32);
            let bp = blurred.get_pixel(x as u32, y as u32);
            let mut new_p = *op;
            for c in 0..4 {
                if c >= n_data {
                    continue;
                }
                let v = bp[c] as f64 * (1.0 - f) + op[c] as f64 * f;
                new_p[c] = v.clamp(0.0, 255.0) as u8;
            }
            result.put_pixel(x as u32, y as u32, new_p);
        }
    }

    Ok(preserve_mode(img, DynamicImage::ImageRgba8(result)))
}
