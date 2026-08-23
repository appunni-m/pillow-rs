// ── ImageChops operations extracted from image.rs execute_op() ──

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::image::{Image, preserve_mode};
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use std::sync::Arc;

#[cfg(feature = "parallel")]
const CHOPS_PARALLEL_PIXEL_THRESHOLD: usize = 512 * 512;

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

fn materialize_chops_other(other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    // Chops.c receives the logical image core. For indexed inputs that means
    // one-byte palette indices, not the visible RGB expansion used by color
    // operations.
    if matches!(other.mode()?.as_str(), "P" | "PA") {
        other.materialize_indices()
    } else {
        other.materialize_for_ops()
    }
}

#[inline]
fn apply_binary_row<F>(left: &[u8], right: &[u8], output: &mut [u8], op: &F)
where
    F: Fn(u8, u8) -> u8,
{
    for ((destination, &left), &right) in output.iter_mut().zip(left).zip(right) {
        *destination = op(left, right);
    }
}

/// Apply a byte-wise binary operation by complete output rows.
///
/// The two input images may have different widths; the caller supplies the
/// source row strides and the already-clipped output width.  Keeping the
/// destination as the only mutable capture lets `par_rows_mut!` prove that
/// concurrent workers own disjoint rows while all source reads remain shared.
fn apply_binary_rows<F>(
    left: &[u8],
    right: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    left_stride: usize,
    right_stride: usize,
    op: F,
) where
    F: Fn(u8, u8) -> u8 + Send + Sync,
{
    let output_stride = width.saturating_mul(channels);
    if width == 0 || height == 0 || output_stride == 0 {
        return;
    }

    #[inline]
    fn apply_row<F>(
        left: &[u8],
        right: &[u8],
        output: &mut [u8],
        y: usize,
        output_stride: usize,
        left_stride: usize,
        right_stride: usize,
        op: &F,
    ) where
        F: Fn(u8, u8) -> u8,
    {
        let left_start = y * left_stride;
        let right_start = y * right_stride;
        apply_binary_row(
            &left[left_start..left_start + output_stride],
            &right[right_start..right_start + output_stride],
            output,
            op,
        );
    }

    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= CHOPS_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            output,
            output_stride,
            height,
            |_row_start, _row_end, y, row| {
                apply_row(
                    left,
                    right,
                    row,
                    y as usize,
                    output_stride,
                    left_stride,
                    right_stride,
                    &op,
                );
            }
        );
    } else {
        for (y, row) in output
            .chunks_exact_mut(output_stride)
            .take(height)
            .enumerate()
        {
            apply_row(
                left,
                right,
                row,
                y,
                output_stride,
                left_stride,
                right_stride,
                &op,
            );
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in output
        .chunks_exact_mut(output_stride)
        .take(height)
        .enumerate()
    {
        apply_row(
            left,
            right,
            row,
            y,
            output_stride,
            left_stride,
            right_stride,
            &op,
        );
    }
}

fn apply_offset_rows(
    source: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    offset_x: i32,
    offset_y: i32,
) {
    let row_stride = width.saturating_mul(4);
    if width == 0 || height == 0 || row_stride == 0 {
        return;
    }
    let source_x = (-(offset_x as i64)).rem_euclid(width as i64) as usize;
    let source_y = (-(offset_y as i64)).rem_euclid(height as i64) as usize;

    #[inline]
    fn apply_row(
        source: &[u8],
        output: &mut [u8],
        width: usize,
        row_stride: usize,
        source_x: usize,
        source_y: usize,
        destination_y: usize,
    ) {
        let source_row = (source_y + destination_y) % (source.len() / row_stride);
        let source_start = source_row * row_stride;
        let source_row = &source[source_start..source_start + row_stride];
        let first_pixels = width - source_x;
        let first_bytes = first_pixels * 4;
        output[..first_bytes].copy_from_slice(&source_row[source_x * 4..]);
        if source_x != 0 {
            output[first_bytes..].copy_from_slice(&source_row[..source_x * 4]);
        }
    }

    #[cfg(feature = "parallel")]
    if width.saturating_mul(height) >= CHOPS_PARALLEL_PIXEL_THRESHOLD {
        crate::par_rows_mut!(
            output,
            row_stride,
            height,
            |_row_start, _row_end, y, row| {
                apply_row(
                    source, row, width, row_stride, source_x, source_y, y as usize,
                );
            }
        );
    } else {
        for (y, row) in output.chunks_exact_mut(row_stride).take(height).enumerate() {
            apply_row(source, row, width, row_stride, source_x, source_y, y);
        }
    }
    #[cfg(not(feature = "parallel"))]
    for (y, row) in output.chunks_exact_mut(row_stride).take(height).enumerate() {
        apply_row(source, row, width, row_stride, source_x, source_y, y);
    }
}

/// Per-channel binary operation.
fn channel_op_binary(
    img: &DynamicImage,
    other: &Arc<Image>,
    op: impl Fn(u8, u8) -> u8 + Send + Sync,
) -> Result<DynamicImage, PilError> {
    let other_img = materialize_chops_other(other)?;
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
    // Pillow returns a valid empty image when both operands share a zero
    // dimension. Keep the ordinary CheckedDims gate for non-empty work while
    // allowing this public boundary case to take the no-pixel early return.
    let mut out = if w == 0 || h == 0 {
        CheckedDims::new_allow_empty(w, h, ch as u8)?.alloc_buffer()
    } else {
        CheckedDims::new(w, h, ch as u8)?.alloc_buffer()
    };

    apply_binary_rows(
        a_bytes, b_bytes, &mut out, w as usize, h as usize, ch, stride_a, stride_b, op,
    );

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
    let other_img = materialize_chops_other(other)?;
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

    let mut out = if w == 0 || h == 0 {
        CheckedDims::new_allow_empty(w, h, ch as u8)?.alloc_buffer()
    } else {
        CheckedDims::new(w, h, ch as u8)?.alloc_buffer()
    };

    apply_binary_rows(
        a_bytes,
        b_bytes,
        &mut out,
        w as usize,
        h as usize,
        ch,
        stride_a,
        stride_b,
        |a, b| lut[a as usize * 256 + b as usize],
    );

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
    let src_rgba = img.to_rgba8();
    let mut result = RgbaImage::new(w, h);
    apply_offset_rows(&src_rgba, &mut result, w as usize, h as usize, x, y);
    preserve_mode(img, DynamicImage::ImageRgba8(result))
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
