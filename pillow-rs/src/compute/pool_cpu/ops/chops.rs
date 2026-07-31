// ── ImageChops operations extracted from image.rs execute_op() ──

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::raster::{DynamicImage, GenericImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use std::sync::Arc;

// ── Blend mode lookup tables (generated from PIL C implementation) ──

static OVERLAY_LUT: [u8; 65536] = {
    let bytes = include_bytes!("../../../../src/ops/lut_overlay.bin");
    let mut arr = [0u8; 65536];
    let mut i = 0;
    while i < 65536 {
        arr[i] = bytes[i];
        i += 1;
    }
    arr
};

static HARD_LIGHT_LUT: [u8; 65536] = {
    let bytes = include_bytes!("../../../../src/ops/lut_hardlight.bin");
    let mut arr = [0u8; 65536];
    let mut i = 0;
    while i < 65536 {
        arr[i] = bytes[i];
        i += 1;
    }
    arr
};

static SOFT_LIGHT_LUT: [u8; 65536] = {
    let bytes = include_bytes!("../../../../src/ops/lut_softlight.bin");
    let mut arr = [0u8; 65536];
    let mut i = 0;
    while i < 65536 {
        arr[i] = bytes[i];
        i += 1;
    }
    arr
};

/// Per-channel binary operation.
fn channel_op_binary(
    img: &DynamicImage,
    other: &Arc<Image>,
    op: impl Fn(u8, u8) -> u8,
) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize_for_ops()?;
    let channels = img.color().channel_count() as usize;
    let other_channels = other_img.color().channel_count() as usize;
    let ch = channels.min(other_channels);

    let (w, h) = (
        img.width().min(other_img.width()),
        img.height().min(other_img.height()),
    );
    let a_bytes = img.as_bytes();
    let b_bytes = other_img.as_bytes();
    let stride_a = img.width() as usize * ch;
    let stride_b = other_img.width() as usize * ch;
    let stride_out = w as usize * ch;

    let mut out = CheckedDims::new(w, h, ch as u8)?.alloc_buffer();

    for y in 0..h as usize {
        for x in 0..w as usize {
            for c in 0..ch {
                let a_idx = y * stride_a + x * ch + c;
                let b_idx = y * stride_b + x * ch + c;
                let o_idx = y * stride_out + x * ch + c;
                out[o_idx] = op(a_bytes[a_idx], b_bytes[b_idx]);
            }
        }
    }

    let result = match ch {
        1 => DynamicImage::ImageLuma8(
            GrayImage::from_raw(w, h, out)
                .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))?,
        ),
        2 => DynamicImage::ImageLumaA8(
            GrayAlphaImage::from_raw(w, h, out)
                .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))?,
        ),
        3 => DynamicImage::ImageRgb8(
            RgbImage::from_raw(w, h, out)
                .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))?,
        ),
        4 => DynamicImage::ImageRgba8(
            RgbaImage::from_raw(w, h, out)
                .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))?,
        ),
        _ => {
            return Err(PilError::ValueError(format!(
                "channel_op_binary: unsupported channel count {}",
                ch
            )));
        }
    };

    Ok(preserve_mode(img, result))
}

/// Per-channel binary operation using a precomputed 256×256 lookup table.
/// The LUT is indexed as LUT[base * 256 + blend] for each channel.
fn channel_op_binary_lut(
    img: &DynamicImage,
    other: &Arc<Image>,
    lut: &[u8; 65536],
) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize_for_ops()?;
    let channels = img.color().channel_count() as usize;
    let other_channels = other_img.color().channel_count() as usize;
    let ch = channels.min(other_channels);

    let (w, h) = (
        img.width().min(other_img.width()),
        img.height().min(other_img.height()),
    );
    let a_bytes = img.as_bytes();
    let b_bytes = other_img.as_bytes();
    let stride_a = img.width() as usize * ch;
    let stride_b = other_img.width() as usize * ch;
    let stride_out = w as usize * ch;

    let mut out = CheckedDims::new(w, h, ch as u8)?.alloc_buffer();

    for y in 0..h as usize {
        for x in 0..w as usize {
            for c in 0..ch {
                let a_idx = y * stride_a + x * ch + c;
                let b_idx = y * stride_b + x * ch + c;
                let o_idx = y * stride_out + x * ch + c;
                out[o_idx] = lut[a_bytes[a_idx] as usize * 256 + b_bytes[b_idx] as usize];
            }
        }
    }

    let result =
        match ch {
            1 => DynamicImage::ImageLuma8(GrayImage::from_raw(w, h, out).ok_or_else(|| {
                PilError::ValueError("channel_op_binary_lut buffer error".into())
            })?),
            2 => {
                DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(w, h, out).ok_or_else(|| {
                    PilError::ValueError("channel_op_binary_lut buffer error".into())
                })?)
            }
            3 => DynamicImage::ImageRgb8(RgbImage::from_raw(w, h, out).ok_or_else(|| {
                PilError::ValueError("channel_op_binary_lut buffer error".into())
            })?),
            4 => DynamicImage::ImageRgba8(RgbaImage::from_raw(w, h, out).ok_or_else(|| {
                PilError::ValueError("channel_op_binary_lut buffer error".into())
            })?),
            _ => {
                return Err(PilError::ValueError(format!(
                    "channel_op_binary_lut: unsupported channel count {}",
                    ch
                )));
            }
        };

    Ok(preserve_mode(img, result))
}

// ── Individual operation functions ──

pub fn op_chops_add(
    img: &DynamicImage,
    other: &Arc<Image>,
    scale: f64,
    offset: f64,
) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| {
        // Pillow 12.2.0 `Chops.c::ImagingChopAdd`: ((a + b) / scale + offset),
        // then CHOP clamps to [0, 255].
        ((a as f64 + b as f64) / scale + offset).clamp(0.0, 255.0) as u8
    })
}

pub fn op_chops_subtract(
    img: &DynamicImage,
    other: &Arc<Image>,
    scale: f64,
    offset: f64,
) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| {
        // Pillow 12.2.0 `Chops.c::ImagingChopSubtract`: ((a - b) / scale +
        // offset), then CHOP clamps to [0, 255].
        ((a as f64 - b as f64) / scale + offset).clamp(0.0, 255.0) as u8
    })
}

pub fn op_chops_multiply(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| ((a as u32 * b as u32) / 255) as u8)
}

pub fn op_chops_screen(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| {
        (255u32 - ((255 - a as u32) * (255 - b as u32) / 255)) as u8
    })
}

pub fn op_chops_darker(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| a.min(b))
}

pub fn op_chops_lighter(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| a.max(b))
}

pub fn op_chops_difference(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| {
        (a as i16 - b as i16).unsigned_abs() as u8
    })
}

pub fn op_chops_overlay(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary_lut(img, other, &OVERLAY_LUT)
}

pub fn op_chops_hard_light(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary_lut(img, other, &HARD_LIGHT_LUT)
}

pub fn op_chops_soft_light(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary_lut(img, other, &SOFT_LIGHT_LUT)
}

pub fn op_chops_add_modulo(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| a.wrapping_add(b))
}

pub fn op_chops_subtract_modulo(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| a.wrapping_sub(b))
}

pub fn op_chops_logical_and(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| a & b)
}

pub fn op_chops_logical_or(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| a | b)
}

pub fn op_chops_logical_xor(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary(img, other, |a, b| a ^ b)
}

pub fn op_chops_constant(img: &DynamicImage, value: u8) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let mut out = GrayImage::new(w, h);
    for p in out.pixels_mut() {
        p[0] = value;
    }
    DynamicImage::ImageLuma8(out)
}

pub fn op_chops_offset(img: &DynamicImage, x: i32, y: i32) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let mut result = DynamicImage::new_rgba8(w, h);
    let src_rgba = img.to_rgba8();
    for py in 0..h {
        for px in 0..w {
            let sx = (px as i32 - x).rem_euclid(w as i32) as u32;
            let sy = (py as i32 - y).rem_euclid(h as i32) as u32;
            result.put_pixel(px, py, *src_rgba.get_pixel(sx, sy));
        }
    }
    preserve_mode(img, result)
}

pub fn op_chops_blend(
    img: &DynamicImage,
    other: &Arc<Image>,
    alpha: f64,
) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize_for_ops()?;
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
                crate::raster::Rgb([
                    // Pillow 12.2.0 `Blend.c::ImagingBlend` interpolates for alpha in
                    // [0, 1] and clips extrapolation results to [0, 255] otherwise.
                    (p1[0] as f64 * (1.0 - alpha) + p2[0] as f64 * alpha).clamp(0.0, 255.0) as u8,
                    (p1[1] as f64 * (1.0 - alpha) + p2[1] as f64 * alpha).clamp(0.0, 255.0) as u8,
                    (p1[2] as f64 * (1.0 - alpha) + p2[2] as f64 * alpha).clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

pub fn op_chops_composite(
    img: &DynamicImage,
    other: &Arc<Image>,
    mask: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    let other_img = other.materialize_for_ops()?;
    let mask_img = mask.materialize_for_ops()?;
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
                crate::raster::Rgb([
                    ((p1[0] as f64 * m + p2[0] as f64 * (1.0 - m)).round()) as u8,
                    ((p1[1] as f64 * m + p2[1] as f64 * (1.0 - m)).round()) as u8,
                    ((p1[2] as f64 * m + p2[2] as f64 * (1.0 - m)).round()) as u8,
                ]),
            );
        }
    }
    Ok(preserve_mode(img, DynamicImage::ImageRgb8(out)))
}

pub fn op_chops_duplicate(img: &DynamicImage) -> DynamicImage {
    img.clone()
}

pub fn op_chops_invert(img: &DynamicImage) -> Result<DynamicImage, PilError> {
    let channels = img.color().channel_count() as usize;
    let (w, h) = (img.width(), img.height());
    let raw = img.as_bytes();
    let mut out = raw.to_vec();
    let stride = w as usize * channels;
    for y in 0..h as usize {
        for x in 0..w as usize {
            for c in 0..channels {
                let idx = y * stride + x * channels + c;
                out[idx] = 255 - out[idx];
            }
        }
    }
    Ok(match channels {
        1 => DynamicImage::ImageLuma8(crate::raster::GrayImage::from_raw(w, h, out).ok_or_else(
            || PilError::InternalError("invert L buffer shape mismatch".to_string()),
        )?),
        2 => DynamicImage::ImageLumaA8(
            crate::raster::GrayAlphaImage::from_raw(w, h, out).ok_or_else(|| {
                PilError::InternalError("invert LA buffer shape mismatch".to_string())
            })?,
        ),
        3 => DynamicImage::ImageRgb8(crate::raster::RgbImage::from_raw(w, h, out).ok_or_else(
            || PilError::InternalError("invert RGB buffer shape mismatch".to_string()),
        )?),
        _ => DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_raw(w, h, out).ok_or_else(
            || PilError::InternalError("invert RGBA buffer shape mismatch".to_string()),
        )?),
    })
}
