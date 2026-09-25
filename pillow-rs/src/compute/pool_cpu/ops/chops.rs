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

fn materialize_chops_other(other: &Arc<Image>) -> Result<Arc<DynamicImage>, PilError> {
    // Chops reads native samples, including palette indices. Retain the
    // immutable cached pixels instead of copying the entire secondary image.
    other.materialized_shared()
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
    _cheap_bytes: bool,
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
    if if _cheap_bytes {
        // Cheap byte arithmetic has little computation to amortize scheduling.
        // The measured crossover is around 4 MiB; larger work uses groups of
        // complete rows while preserving each source's independent stride.
        output.len() >= 4 * 1024 * 1024
    } else {
        width.saturating_mul(height) >= CHOPS_PARALLEL_PIXEL_THRESHOLD
    } {
        let rows_per_task = if _cheap_bytes { height.min(32) } else { 1 };
        crate::par_rows_mut!(
            output,
            output_stride * rows_per_task,
            height.div_ceil(rows_per_task),
            |_row_start, _row_end, tile, rows| {
                for (local_y, row) in rows.chunks_exact_mut(output_stride).enumerate() {
                    apply_row(
                        left,
                        right,
                        row,
                        tile as usize * rows_per_task + local_y,
                        output_stride,
                        left_stride,
                        right_stride,
                        &op,
                    );
                }
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

/// Per-channel binary operation.
fn channel_op_binary(
    img: &DynamicImage,
    other: &Arc<Image>,
    op: impl Fn(u8, u8) -> u8 + Send + Sync,
) -> Result<DynamicImage, PilError> {
    channel_op_binary_with_policy(img, other, op, false)
}

/// The cheap-byte policy trades row scheduling for contiguous work until the
/// measured byte-size crossover, without changing the arithmetic or layout.
fn channel_op_binary_with_policy(
    img: &DynamicImage,
    other: &Arc<Image>,
    op: impl Fn(u8, u8) -> u8 + Send + Sync,
    cheap_bytes: bool,
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
        a_bytes,
        b_bytes,
        &mut out,
        w as usize,
        h as usize,
        ch,
        stride_a,
        stride_b,
        op,
        cheap_bytes,
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
        false,
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
    let scale = scale as f32;
    let offset = offset as f32;
    if scale == 1.0 && offset == 0.0 {
        return channel_op_binary(img, other, u8::saturating_add);
    }
    channel_op_binary(img, other, |a, b| {
        // ImagingChopAdd takes float scale and int offset. Both the division
        // and addition round in float32 before truncation; f64 changes bytes.
        ((f32::from(a) + f32::from(b)) / scale + offset).clamp(0.0, 255.0) as u8
    })
}

pub fn op_chops_subtract(
    img: &DynamicImage,
    other: &Arc<Image>,
    scale: f64,
    offset: f64,
) -> Result<DynamicImage, PilError> {
    let scale = scale as f32;
    let offset = offset as f32;
    if scale == 1.0 && offset == 0.0 {
        return channel_op_binary_with_policy(img, other, u8::saturating_sub, true);
    }
    channel_op_binary(img, other, |a, b| {
        // Subtraction uses the same float32 scale/division/offset ordering.
        ((f32::from(a) - f32::from(b)) / scale + offset).clamp(0.0, 255.0) as u8
    })
}

pub fn op_chops_multiply(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary_with_policy(img, other, |a, b| ((a as u32 * b as u32) / 255) as u8, true)
}

pub fn op_chops_screen(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary_with_policy(
        img,
        other,
        |a, b| (255u32 - ((255 - a as u32) * (255 - b as u32) / 255)) as u8,
        true,
    )
}

pub fn op_chops_darker(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary_with_policy(img, other, |a, b| a.min(b), true)
}

pub fn op_chops_lighter(img: &DynamicImage, other: &Arc<Image>) -> Result<DynamicImage, PilError> {
    channel_op_binary_with_policy(img, other, |a, b| a.max(b), true)
}

pub fn op_chops_difference(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary_with_policy(img, other, u8::abs_diff, true)
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
    channel_op_binary_with_policy(img, other, u8::wrapping_add, true)
}

pub fn op_chops_subtract_modulo(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    channel_op_binary_with_policy(img, other, u8::wrapping_sub, true)
}

pub fn op_chops_logical_and(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    // Mode 1 can retain noncanonical samples through putdata/putpixel. Pillow
    // tests each stored byte for truth and writes a canonical 0/255 result.
    channel_op_binary(img, other, |a, b| u8::from(a != 0 && b != 0) * 255)
}

pub fn op_chops_logical_or(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    // Logical Chops canonicalizes truth even when public writes retained
    // nonzero mode-1 samples other than 255.
    channel_op_binary_with_policy(img, other, |a, b| u8::from((a | b) != 0) * 255, true)
}

pub fn op_chops_logical_xor(
    img: &DynamicImage,
    other: &Arc<Image>,
) -> Result<DynamicImage, PilError> {
    // XOR compares truth, not stored bits: nonzero samples 1 and 2 are equal
    // as booleans and therefore produce zero.
    channel_op_binary_with_policy(img, other, |a, b| u8::from((a != 0) ^ (b != 0)) * 255, true)
}

pub fn op_chops_constant(img: &DynamicImage, value: u8) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    // The output is independent of the source pixels. Constructing it with
    // the final pixel removes the zero-fill allocation followed by a second
    // full-frame write while preserving the new single-band L result.
    let out = GrayImage::from_pixel(w, h, crate::raster::Luma([value]));
    DynamicImage::ImageLuma8(out)
}

pub fn op_chops_offset(img: &DynamicImage, x: i32, y: i32, mode: Option<&str>) -> DynamicImage {
    img.offset_with_mode(x, y, mode)
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
