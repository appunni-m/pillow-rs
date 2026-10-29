use crate::error::PilError;
use crate::raster::{
    DynamicImage, GenericImageView, GrayAlphaImage, GrayImage, RgbImage, RgbaImage,
};

use super::projective::cubic_sample;
use super::scalar::affine_nearest_fixed;

pub(super) fn transform_affine_luma_nearest(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    affine: [f64; 6],
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<Option<DynamicImage>, PilError> {
    let DynamicImage::ImageLuma8(source) = img else {
        return Ok(None);
    };
    let Some([step_x, step_y, origin_x, step_x_y, step_y_y, origin_y]) =
        affine_nearest_fixed(affine, dst_w, dst_h)
    else {
        return Ok(None);
    };
    let width = usize::try_from(dst_w)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    let height = usize::try_from(dst_h)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let output_len = width
        .checked_mul(height)
        .ok_or_else(|| PilError::ValueError("transform output is too large".into()))?;
    let source_width = usize::try_from(source.width())
        .map_err(|_| PilError::ValueError("transform source is too large".into()))?;
    let source_height = i64::from(source.height());
    let fill = fill.map_or(0, |color| color.0);
    let raw = source.as_raw();
    let mut output = vec![fill; output_len];

    for (y, row) in output.chunks_exact_mut(width).enumerate() {
        let mut sx = origin_x + y as i64 * step_y;
        let mut sy = origin_y + y as i64 * step_y_y;
        for pixel in row {
            let ix = sx >> 16;
            let iy = sy >> 16;
            if ix >= 0 && ix < source_width as i64 && iy >= 0 && iy < source_height {
                *pixel = raw[iy as usize * source_width + ix as usize];
            }
            sx += step_x;
            sy += step_x_y;
        }
    }

    let image = GrayImage::from_raw(dst_w, dst_h, output).ok_or_else(|| {
        PilError::InternalError("transform affine L buffer shape mismatch".into())
    })?;
    Ok(Some(DynamicImage::ImageLuma8(image)))
}

pub(super) fn transform_affine_luma_alpha_nearest(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    affine: [f64; 6],
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<Option<DynamicImage>, PilError> {
    let DynamicImage::ImageLumaA8(source) = img else {
        return Ok(None);
    };
    let Some([step_x, step_y, origin_x, step_x_y, step_y_y, origin_y]) =
        affine_nearest_fixed(affine, dst_w, dst_h)
    else {
        return Ok(None);
    };
    let width = usize::try_from(dst_w)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    let height = usize::try_from(dst_h)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let output_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(2))
        .ok_or_else(|| PilError::ValueError("transform output is too large".into()))?;
    let source_width = usize::try_from(source.width())
        .map_err(|_| PilError::ValueError("transform source is too large".into()))?;
    let source_height = i64::from(source.height());
    let fill = fill.map_or([0, 0], |color| [color.0, color.1]);
    let raw = source.as_raw();
    let mut output = vec![0; output_len];

    for (y, row) in output.chunks_exact_mut(width * 2).enumerate() {
        let mut sx = origin_x + y as i64 * step_y;
        let mut sy = origin_y + y as i64 * step_y_y;
        for pixel in row.chunks_exact_mut(2) {
            let ix = sx >> 16;
            let iy = sy >> 16;
            if ix >= 0 && ix < source_width as i64 && iy >= 0 && iy < source_height {
                let source_start = (iy as usize * source_width + ix as usize) * 2;
                pixel.copy_from_slice(&raw[source_start..source_start + 2]);
            } else {
                pixel.copy_from_slice(&fill);
            }
            sx += step_x;
            sy += step_x_y;
        }
    }

    let image = GrayAlphaImage::from_raw(dst_w, dst_h, output).ok_or_else(|| {
        PilError::InternalError("transform affine LA buffer shape mismatch".into())
    })?;
    Ok(Some(DynamicImage::ImageLumaA8(image)))
}

pub(super) fn transform_affine_rgb_nearest(
    img: &DynamicImage,
    dst_w: u32,
    dst_h: u32,
    affine: [f64; 6],
    fill: Option<(u8, u8, u8, u8)>,
) -> Result<Option<DynamicImage>, PilError> {
    let DynamicImage::ImageRgb8(source) = img else {
        return Ok(None);
    };
    let Some([step_x, step_y, origin_x, step_x_y, step_y_y, origin_y]) =
        affine_nearest_fixed(affine, dst_w, dst_h)
    else {
        return Ok(None);
    };
    let width = usize::try_from(dst_w)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    let height = usize::try_from(dst_h)
        .map_err(|_| PilError::ValueError("transform output is too large".into()))?;
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let output_len = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| PilError::ValueError("transform output is too large".into()))?;
    let source_width = usize::try_from(source.width())
        .map_err(|_| PilError::ValueError("transform source is too large".into()))?;
    let source_height = i64::from(source.height());
    let fill = fill.map_or([0, 0, 0], |color| [color.0, color.1, color.2]);
    let raw = source.as_raw();
    let mut output = vec![0; output_len];

    for (y, row) in output.chunks_exact_mut(width * 3).enumerate() {
        let mut sx = origin_x + y as i64 * step_y;
        let mut sy = origin_y + y as i64 * step_y_y;
        for pixel in row.chunks_exact_mut(3) {
            let ix = sx >> 16;
            let iy = sy >> 16;
            if ix >= 0 && ix < source_width as i64 && iy >= 0 && iy < source_height {
                let source_start = (iy as usize * source_width + ix as usize) * 3;
                pixel.copy_from_slice(&raw[source_start..source_start + 3]);
            } else {
                pixel.copy_from_slice(&fill);
            }
            sx += step_x;
            sy += step_x_y;
        }
    }

    let image = RgbImage::from_raw(dst_w, dst_h, output).ok_or_else(|| {
        PilError::InternalError("transform affine RGB buffer shape mismatch".into())
    })?;
    Ok(Some(DynamicImage::ImageRgb8(image)))
}

#[inline(always)]
fn transform_affine_fixed_nearest_pixels(
    raw: &[u8],
    output: &mut [u8],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    channels: usize,
    steps: [i64; 6],
    fill_color: (u8, u8, u8, u8),
) {
    let (sw, sh) = source_dimensions;
    let (dst_w, dst_h) = destination_dimensions;
    let [a, b, c, d, e, f] = steps;
    let fill = [fill_color.0, fill_color.1, fill_color.2, fill_color.3];
    for y in 0..dst_h {
        let mut sx = c + i64::from(y) * b;
        let mut sy = f + i64::from(y) * e;
        for x in 0..dst_w {
            let ix = sx >> 16;
            let iy = sy >> 16;
            let index = (y as usize * dst_w as usize + x as usize) * channels;
            let pixel = &mut output[index..index + channels];
            if ix >= 0 && ix < i64::from(sw) && iy >= 0 && iy < i64::from(sh) {
                let source = (iy as usize * sw as usize + ix as usize) * channels;
                pixel.copy_from_slice(&raw[source..source + channels]);
            } else {
                pixel.copy_from_slice(&fill[..channels]);
            }
            sx += a;
            sy += d;
        }
    }
}

#[inline(always)]
fn transform_affine_generic_pixels(
    raw: &[u8],
    output: &mut [u8],
    source_dimensions: (u32, u32),
    destination_dimensions: (u32, u32),
    affine: [f64; 6],
    channels: usize,
    fill_color: (u8, u8, u8, u8),
    nearest: bool,
    bicubic: bool,
) {
    let (dst_w, dst_h) = destination_dimensions;
    let [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f] = affine;
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            // Geometry.c's affine_transform() evaluates destination pixel
            // centers before applying the matrix. The nearest filter then
            // truncates non-negative coordinates, while interpolating filters
            // subtract 0.5 inside their own filter prologue.
            // Keep the same grouping as Geometry.c's affine_transform()
            // expression as compiled by clang: the first product is fused
            // with the second product, then the translation is added. A
            // reassociated plain expression can move an edge coordinate by
            // one ULP after an LA/RGBA premultiplied round trip.
            let sx = aff_a.mul_add(dx as f64 + 0.5, aff_b * (dy as f64 + 0.5)) + aff_c;
            let sy = aff_d.mul_add(dx as f64 + 0.5, aff_e * (dy as f64 + 0.5)) + aff_f;
            let out_idx = (dy * dst_w + dx) as usize * channels;
            transform_affine_generic_pixel(
                raw,
                &mut output[out_idx..out_idx + channels],
                source_dimensions,
                sx,
                sy,
                channels,
                fill_color,
                nearest,
                bicubic,
            );
        }
    }
}

#[inline(always)]
fn transform_affine_generic_pixel(
    raw: &[u8],
    output_pixel: &mut [u8],
    source_dimensions: (u32, u32),
    sx: f64,
    sy: f64,
    channels: usize,
    fill_color: (u8, u8, u8, u8),
    nearest: bool,
    bicubic: bool,
) {
    let (sw, sh) = source_dimensions;
    if nearest {
        // Matches Geometry.c's COORD(): negative coordinates are rejected
        // and non-negative values use C's truncating cast.
        let ix = if sx < 0.0 { -1 } else { sx as i64 };
        let iy = if sy < 0.0 { -1 } else { sy as i64 };
        if ix >= 0 && ix < sw as i64 && iy >= 0 && iy < sh as i64 {
            let in_idx = (iy as u32 * sw + ix as u32) as usize * channels;
            output_pixel.copy_from_slice(&raw[in_idx..in_idx + channels]);
        } else {
            for ch in 0..channels.min(4) {
                output_pixel[ch] = match ch {
                    0 => fill_color.0,
                    1 => fill_color.1,
                    2 => fill_color.2,
                    _ => fill_color.3,
                };
            }
        }
    } else if sw > 0 && sh > 0 && sx >= 0.0 && sx < sw as f64 && sy >= 0.0 && sy < sh as f64 {
        // Matches Geometry.c's BILINEAR_HEAD(): bounds are checked in center
        // space, then the filter moves to surrounding sample corners.
        let sample_x = sx - 0.5;
        let sample_y = sy - 0.5;
        if bicubic {
            // Pillow's BICUBIC_BODY uses four horizontally clipped taps and a
            // sequential vertical fallback: an out-of-range row repeats the
            // previous row rather than independently clamping every row.
            let x_floor = sample_x.floor() as i64;
            let y_floor = sample_y.floor() as i64;
            let x = x_floor - 1;
            let y = y_floor - 1;
            let fx = sample_x - x_floor as f64;
            let fy = sample_y - y_floor as f64;
            for ch in 0..channels {
                let mut rows = [0.0; 4];
                for row in 0..4 {
                    let yy = y + row as i64;
                    if row == 0 || (yy >= 0 && yy < sh as i64) {
                        let y_index = yy.clamp(0, sh as i64 - 1) as usize;
                        let samples = [
                            raw[(y_index * sw as usize + x.clamp(0, sw as i64 - 1) as usize)
                                * channels
                                + ch] as f64,
                            raw[(y_index * sw as usize + (x + 1).clamp(0, sw as i64 - 1) as usize)
                                * channels
                                + ch] as f64,
                            raw[(y_index * sw as usize + (x + 2).clamp(0, sw as i64 - 1) as usize)
                                * channels
                                + ch] as f64,
                            raw[(y_index * sw as usize + (x + 3).clamp(0, sw as i64 - 1) as usize)
                                * channels
                                + ch] as f64,
                        ];
                        rows[row] = cubic_sample(samples, fx);
                    } else {
                        rows[row] = rows[row - 1];
                    }
                }
                output_pixel[ch] = cubic_sample(rows, fy).clamp(0.0, 255.0) as u8;
            }
        } else {
            let x_floor = sample_x.floor() as i64;
            let y_floor = sample_y.floor() as i64;
            let x0 = x_floor.clamp(0, sw as i64 - 1) as u32;
            let y0 = y_floor.clamp(0, sh as i64 - 1) as u32;
            let x1 = (x_floor + 1).clamp(0, sw as i64 - 1) as u32;
            let y1 = (y_floor + 1).clamp(0, sh as i64 - 1) as u32;
            let fx = sample_x - x_floor as f64;
            let fy = sample_y - y_floor as f64;
            for ch in 0..channels {
                let p00 = raw[(y0 * sw + x0) as usize * channels + ch] as f64;
                let p10 = raw[(y0 * sw + x1) as usize * channels + ch] as f64;
                let p01 = raw[(y1 * sw + x0) as usize * channels + ch] as f64;
                let p11 = raw[(y1 * sw + x1) as usize * channels + ch] as f64;
                // Geometry.c interpolates each row and then interpolates
                // those results; a four-term sum can differ by one ULP.
                let top = (p10 - p00).mul_add(fx, p00);
                let bottom = (p11 - p01).mul_add(fx, p01);
                let value = (bottom - top).mul_add(fy, top);
                // Pillow's byte transform truncates toward zero. Rounding
                // makes exact halfway samples one value too high.
                output_pixel[ch] = value.clamp(0.0, 255.0) as u8;
            }
        }
    } else {
        for ch in 0..channels.min(4) {
            output_pixel[ch] = match ch {
                0 => fill_color.0,
                1 => fill_color.1,
                2 => fill_color.2,
                _ => fill_color.3,
            };
        }
    }
}

/// Apply an affine transform working on the native number of channels.
/// When `nearest` is true, uses nearest-neighbor sampling.
pub(super) fn transform_affine_generic(
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
    bicubic: bool,
) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let raw = img.as_bytes();
    let (sw, sh) = img.dimensions();
    let fill_color = fill.unwrap_or_else(|| {
        if matches!(
            img.color(),
            crate::raster::ColorType::La8 | crate::raster::ColorType::Rgba8
        ) {
            (0, 0, 0, 0)
        } else {
            (0, 0, 0, 255)
        }
    });

    let mut out = vec![0u8; (dst_w * dst_h) as usize * channels];
    let affine = [aff_a, aff_b, aff_c, aff_d, aff_e, aff_f];

    if nearest && let Some(steps) = affine_nearest_fixed(affine, dst_w, dst_h) {
        transform_affine_fixed_nearest_pixels(
            raw,
            &mut out,
            (sw, sh),
            (dst_w, dst_h),
            channels,
            steps,
            fill_color,
        );
        return crate::image_utils::raw_bytes_to_image(dst_w, dst_h, out, channels);
    }

    transform_affine_generic_pixels(
        raw,
        &mut out,
        (sw, sh),
        (dst_w, dst_h),
        affine,
        channels,
        fill_color,
        nearest,
        bicubic,
    );

    Ok(match channels {
        1 => DynamicImage::ImageLuma8(GrayImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform_affine L buffer shape mismatch".to_string())
        })?),
        2 => DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(dst_w, dst_h, out).ok_or_else(
            || PilError::InternalError("transform_affine LA buffer shape mismatch".to_string()),
        )?),
        3 => DynamicImage::ImageRgb8(RgbImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform_affine RGB buffer shape mismatch".to_string())
        })?),
        4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(dst_w, dst_h, out).ok_or_else(|| {
            PilError::InternalError("transform_affine RGBA buffer shape mismatch".to_string())
        })?),
        _ => unreachable!(),
    })
}
