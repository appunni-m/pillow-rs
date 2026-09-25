// ── Enhance operations extracted from image.rs execute_op() ──

use crate::error::PilError;
use crate::image::preserve_mode;
use crate::raster::{DynamicImage, GrayAlphaImage, RgbaImage};

#[cfg(feature = "parallel")]
const ENHANCE_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

fn apply_enhance_rows<F>(
    bytes: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    transform: F,
) where
    F: Fn(usize, &mut [u8]) + Send + Sync,
{
    if bytes.is_empty() || width == 0 || height == 0 {
        return;
    }
    let stride = width.saturating_mul(channels);
    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= ENHANCE_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(bytes, stride, height, |_row_start, _row_end, y, row| {
            transform(y as usize, row);
        });
    } else {
        for (y, row) in bytes.chunks_exact_mut(stride).take(height).enumerate() {
            transform(y, row);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in bytes.chunks_exact_mut(stride).take(height).enumerate() {
        transform(y, row);
    }
}

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
    // Pillow's ImageEnhance.Brightness blends the source with a black image;
    // factor 1.0 is therefore an exact copy. Native 8-bit layouts already
    // have the requested result, so avoid widening (L/LA) or scanning every
    // byte through the floating-point multiply. Keep typed 16-bit/float
    // inputs on the conversion path below: their public enhancer contract is
    // distinct from the byte-oriented layouts represented here.
    if factor == 1.0
        && matches!(
            mode,
            None | Some("L" | "LA" | "RGB" | "RGBA" | "RGBa" | "RGBX" | "CMYK" | "HSV" | "YCbCr")
        )
        && matches!(
            img,
            DynamicImage::ImageLuma8(_)
                | DynamicImage::ImageLumaA8(_)
                | DynamicImage::ImageRgb8(_)
                | DynamicImage::ImageRgba8(_)
        )
    {
        return Ok(img.clone());
    }
    // CMYK mode: stored as RGBA8 (C→R, M→G, Y→B, K→A). Operate on all 4 channels.
    if mode == Some("CMYK") {
        let mut rgba = img.to_rgba8();
        let f = factor;
        let (width, height) = rgba.dimensions();
        apply_enhance_rows(
            rgba.as_mut(),
            width as usize,
            height as usize,
            4,
            |_y, row| {
                for pixel in row.chunks_exact_mut(4) {
                    for channel in pixel.iter_mut() {
                        *channel = (*channel as f64 * f).clamp(0.0, 255.0) as u8;
                    }
                }
            },
        );
        return Ok(DynamicImage::ImageRgba8(rgba));
    }
    if matches!(
        img,
        DynamicImage::ImageLumaA8(_) | DynamicImage::ImageRgba8(_)
    ) {
        let mut rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        apply_enhance_rows(
            rgba.as_mut(),
            width as usize,
            height as usize,
            4,
            |_y, row| {
                for pixel in row.chunks_exact_mut(4) {
                    for channel in pixel.iter_mut().take(3) {
                        *channel = (*channel as f64 * factor).clamp(0.0, 255.0) as u8;
                    }
                }
            },
        );
        return Ok(preserve_alpha_result(img, rgba));
    }
    let mut rgb = img.to_rgb8();
    let f = factor;
    let (width, height) = rgb.dimensions();
    apply_enhance_rows(
        rgb.as_mut(),
        width as usize,
        height as usize,
        3,
        |_y, row| {
            for pixel in row.chunks_exact_mut(3) {
                for channel in pixel.iter_mut() {
                    *channel = (*channel as f64 * f).clamp(0.0, 255.0) as u8;
                }
            }
        },
    );
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(rgb)))
}

pub fn op_enhance_contrast(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if (factor as f32).is_finite()
        && let Some(base) = crate::ops::enhance::contrast_base(img, mode)
    {
        let lut = crate::ops::enhance::contrast_lut(&base, factor);
        return super::effects::op_eval(img, &lut);
    }
    // Rare logical modes use the same conversion and blend contract as the
    // public constructor. Keep those nested operations on the requested CPU.
    let image = crate::Image::from_dynamic(img.clone(), mode.map(str::to_owned))
        .use_backend(crate::compute::Backend::Cpu);
    let base = image.contrast_degenerate()?;
    crate::ops::module_fns::blend(&base, &image, factor)?
        .use_backend(crate::compute::Backend::Cpu)
        .materialize_for_ops()
}

#[cfg(test)]
mod tests {
    use crate::Image;

    #[test]
    fn contrast_preserves_empty_cmyk_images() {
        for size in [(0, 0), (0, 3), (3, 0)] {
            for factor in [0.0, 0.5, 1.0, 2.0] {
                let image =
                    Image::new(size.0, size.1, "CMYK", (0, 0, 0, 0)).expect("empty CMYK image");
                let enhanced = image
                    .enhance_contrast(factor)
                    .expect("Contrast queues for empty CMYK image");

                assert_eq!(enhanced.size().expect("empty size"), size);
                assert_eq!(enhanced.mode().expect("CMYK mode"), "CMYK");
                assert!(enhanced.tobytes().expect("empty bytes").is_empty());
            }
        }
    }
}

pub fn op_enhance_color_saturation(
    img: &DynamicImage,
    factor: f64,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let channels = match img {
        DynamicImage::ImageLuma8(_) => 1,
        DynamicImage::ImageLumaA8(_) => 2,
        DynamicImage::ImageRgb8(_) => 3,
        DynamicImage::ImageRgba8(_) => 4,
        _ => return Err(PilError::ValueError("image has wrong mode".into())),
    };
    let source = img.as_bytes();
    let mut result = img.clone();
    let output = result
        .as_bytes_mut()
        .ok_or_else(|| PilError::ValueError("image has wrong mode".into()))?;
    let factor = factor as f32;
    apply_enhance_rows(
        output,
        img.width() as usize,
        img.height() as usize,
        channels,
        |row_index, row| {
            let start = row_index * img.width() as usize * channels;
            let end = start + row.len();
            for (pixel, original) in row
                .chunks_exact_mut(channels)
                .zip(source[start..end].chunks_exact(channels))
            {
                let base = crate::ops::enhance::color_pixel_base(original, mode);
                for channel in 0..channels {
                    let base = f32::from(base[channel]);
                    // Image.blend narrows factor to f32 and contracts the multiply
                    // and addition. Even equal alpha samples become zero for NaN.
                    pixel[channel] = factor
                        .mul_add(f32::from(original[channel]) - base, base)
                        .clamp(0.0, 255.0) as u8;
                }
            }
        },
    );
    Ok(result)
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
