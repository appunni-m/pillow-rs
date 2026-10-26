//! Native SIMD kernels for ImageChops byte operations.

#[cfg(feature = "parallel")]
use super::super::simd_rows_mut_serial;
use super::super::{simd_div255, simd_pack_u16x16};
use super::native_chops_layout;
use crate::image::preserve_mode;
use crate::raster::{DynamicImage, GenericImageView};
use wide::{f32x8, u8x16, u16x16};

#[inline]
fn simd_div255_u16x16(value: u16x16) -> u16x16 {
    let incremented = value + u16x16::splat(1);
    (incremented + (incremented >> 8u32)) >> 8u32
}

/// Multiply/screen operate on stored bytes, independently of row or channel.
#[inline(always)]
fn native_chops_blend_block(left: [u8; 16], right: [u8; 16], screen: bool) -> [u8; 16] {
    const MAX: u16x16 = u16x16::splat(255);
    let left = u16x16::from(u8x16::new(left));
    let right = u16x16::from(u8x16::new(right));
    let values = if screen {
        MAX - simd_div255((MAX - left) * (MAX - right))
    } else {
        simd_div255(left * right)
    };
    simd_pack_u16x16(values).to_array()
}

/// Load complete blocks directly; only the final partial block needs padding.
fn apply_native_blend_rows(
    left: &[u8],
    right: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    screen: bool,
) -> bool {
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(output.len())
        || left.len() != output.len()
        || right.len() != output.len()
    {
        return false;
    }
    let vector_blocks = left.len().div_ceil(16);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }
    let apply_row = |left_row: &[u8], right_row: &[u8], output_row: &mut [u8]| {
        let mut left_chunks = left_row.chunks_exact(16);
        let mut right_chunks = right_row.chunks_exact(16);
        let mut output_chunks = output_row.chunks_exact_mut(16);
        for ((left, right), output) in left_chunks
            .by_ref()
            .zip(right_chunks.by_ref())
            .zip(output_chunks.by_ref())
        {
            output.copy_from_slice(&native_chops_blend_block(
                left.try_into().expect("complete vector"),
                right.try_into().expect("complete vector"),
                screen,
            ));
        }
        let remainder = output_chunks.into_remainder();
        if !remainder.is_empty() {
            let mut left = [0; 16];
            let mut right = [0; 16];
            left[..remainder.len()].copy_from_slice(left_chunks.remainder());
            right[..remainder.len()].copy_from_slice(right_chunks.remainder());
            remainder
                .copy_from_slice(&native_chops_blend_block(left, right, screen)[..remainder.len()]);
        }
    };

    #[cfg(feature = "parallel")]
    if output.len() >= 4 * 1024 * 1024 {
        const TILE_BYTES: usize = 64 * 1024;
        let tiles = output.len().div_ceil(TILE_BYTES);
        simd_rows_mut_serial!(output, TILE_BYTES, tiles, |start, _end, _tile, bytes| {
            let end = start + bytes.len();
            apply_row(&left[start..end], &right[start..end], bytes);
        });
    } else {
        apply_row(left, right, output);
    }
    #[cfg(not(feature = "parallel"))]
    apply_row(left, right, output);
    true
}

/// Apply the same blend kernel while reusing the owned destination buffer.
/// `left` is mutated only after each sixteen-byte source block has been
/// copied into a value-owned vector, so the in-place path is equivalent to the
/// allocating path even when the output aliases the left operand.
pub(super) fn apply_native_blend_rows_in_place(
    left: &mut [u8],
    right: &[u8],
    width: usize,
    height: usize,
    channels: usize,
    screen: bool,
) -> bool {
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(left.len()) || right.len() != left.len() {
        return false;
    }
    let vector_blocks = left.len().div_ceil(16);
    if vector_blocks != 0 {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    }

    let apply_row = |left_row: &mut [u8], right_row: &[u8]| {
        let mut left_chunks = left_row.chunks_exact_mut(16);
        let mut right_chunks = right_row.chunks_exact(16);
        for (left, right) in left_chunks.by_ref().zip(right_chunks.by_ref()) {
            let values = native_chops_blend_block(
                (&*left).try_into().expect("complete vector"),
                right.try_into().expect("complete vector"),
                screen,
            );
            left.copy_from_slice(&values);
        }
        let remainder = left_chunks.into_remainder();
        if !remainder.is_empty() {
            let mut left = [0; 16];
            let mut right = [0; 16];
            left[..remainder.len()].copy_from_slice(remainder);
            right[..remainder.len()].copy_from_slice(right_chunks.remainder());
            remainder
                .copy_from_slice(&native_chops_blend_block(left, right, screen)[..remainder.len()]);
        }
    };
    #[cfg(feature = "parallel")]
    if left.len() >= 4 * 1024 * 1024 {
        const TILE_BYTES: usize = 64 * 1024;
        let tiles = left.len().div_ceil(TILE_BYTES);
        simd_rows_mut_serial!(left, TILE_BYTES, tiles, |start, _end, _tile, bytes| {
            let end = start + bytes.len();
            apply_row(bytes, &right[start..end]);
        });
    } else {
        apply_row(left, right);
    }
    #[cfg(not(feature = "parallel"))]
    apply_row(left, right);
    crate::compute::record_pipeline_operation_path("vector");
    true
}

pub(super) fn native_chops_blend(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    screen: bool,
) -> Option<DynamicImage> {
    let channels = native_chops_pair_channels(img, other, mode)?;
    let Some(expected_len) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return None;
    };
    if img.as_bytes().len() != expected_len || other.as_bytes().len() != expected_len {
        return None;
    }
    if expected_len == 0 {
        // Pillow returns a typed zero-pixel result without entering the blend
        // loop. The source clone preserves the logical mode at the pipeline
        // boundary and records a genuine native no-work path.
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    let mut output = vec![0u8; expected_len];
    if !apply_native_blend_rows(
        img.as_bytes(),
        other.as_bytes(),
        &mut output,
        img.width() as usize,
        img.height() as usize,
        channels,
        screen,
    ) {
        return None;
    }
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
            .ok()
            .map(|result| preserve_mode(img, result));
    if result.is_some() {
        crate::compute::record_pipeline_operation_path("vector");
    }
    result
}

/// Apply an exact byte-wise Chops operation without widening the native image.
///
/// These formulas are lane-local and preserve Pillow's byte semantics exactly:
/// min/max, absolute difference, modulo add/subtract, and logical operations
/// do not need the widening and rounding used by multiply/screen. The helper
/// pads the final vector block and returns `None` when the logical mode does
/// not match a supported native byte layout. Padding is never written back,
/// so short indexed rows do not need a scalar-only escape hatch.
fn native_chops_bytewise<F, G>(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    vector_op: F,
    _scalar_op: G,
) -> Option<DynamicImage>
where
    F: Fn(u8x16, u8x16) -> u8x16 + Send + Sync,
    G: Fn(u8, u8) -> u8 + Send + Sync,
{
    let channels = native_chops_pair_channels(img, other, mode)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    if left.len() != right.len() || !left.len().is_multiple_of(channels) {
        return None;
    }
    let Some(expected_len) = img
        .width()
        .checked_mul(img.height())
        .and_then(|pixels| pixels.checked_mul(channels as u32))
    else {
        return None;
    };
    if expected_len as usize != left.len() {
        return None;
    }
    if left.is_empty() {
        // Empty Chops is a valid native zero-work operation. Do not pass the
        // empty buffer through raw image reconstruction, whose normal typed
        // constructor intentionally requires at least one pixel.
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    let mut output = vec![0u8; left.len()];
    let mut left_chunks = left.chunks_exact(16);
    let mut right_chunks = right.chunks_exact(16);
    let mut output_chunks = output.chunks_exact_mut(16);
    for ((left_chunk, right_chunk), output_chunk) in left_chunks
        .by_ref()
        .zip(right_chunks.by_ref())
        .zip(output_chunks.by_ref())
    {
        let left = u8x16::new(left_chunk.try_into().expect("16-byte exact chunk"));
        let right = u8x16::new(right_chunk.try_into().expect("16-byte exact chunk"));
        output_chunk.copy_from_slice(&vector_op(left, right).to_array());
    }
    let remainder = output_chunks.into_remainder();
    if !remainder.is_empty() {
        let mut left_padded = [0u8; 16];
        let mut right_padded = [0u8; 16];
        left_padded[..remainder.len()].copy_from_slice(left_chunks.remainder());
        right_padded[..remainder.len()].copy_from_slice(right_chunks.remainder());
        let values = vector_op(u8x16::new(left_padded), u8x16::new(right_padded));
        remainder.copy_from_slice(&values.to_array()[..remainder.len()]);
    }
    crate::compute::record_pipeline_operation_vector_blocks(output.len().div_ceil(16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result))
}

/// Apply a lane-local Chops operation to an owned destination buffer.  The
/// secondary operand remains immutable, so this removes only the unnecessary
/// destination allocation and never changes the public two-image contract.
pub(super) fn native_chops_bytewise_in_place<F, G>(
    img: &mut DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    vector_op: F,
    _scalar_op: G,
) -> bool
where
    F: Fn(u8x16, u8x16) -> u8x16,
    G: Fn(u8, u8) -> u8,
{
    let channels = match native_chops_pair_channels(&*img, other, mode) {
        Some(channels) => channels,
        None => return false,
    };
    let right = other.as_bytes();
    let width = img.width() as usize;
    let height = img.height() as usize;
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    let Some(left) = img.as_bytes_mut() else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(left.len()) || right.len() != left.len() {
        return false;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return true;
    }
    let mut left_chunks = left.chunks_exact_mut(16);
    let mut right_chunks = right.chunks_exact(16);
    for (left_chunk, right_chunk) in left_chunks.by_ref().zip(right_chunks.by_ref()) {
        let left = u8x16::new((&*left_chunk).try_into().expect("16-byte exact chunk"));
        let right = u8x16::new(right_chunk.try_into().expect("16-byte exact chunk"));
        left_chunk.copy_from_slice(&vector_op(left, right).to_array());
    }
    let remainder = left_chunks.into_remainder();
    if !remainder.is_empty() {
        let mut left_padded = [0u8; 16];
        let mut right_padded = [0u8; 16];
        left_padded[..remainder.len()].copy_from_slice(remainder);
        right_padded[..remainder.len()].copy_from_slice(right_chunks.remainder());
        let values = vector_op(u8x16::new(left_padded), u8x16::new(right_padded));
        remainder.copy_from_slice(&values.to_array()[..remainder.len()]);
    }
    crate::compute::record_pipeline_operation_vector_blocks(left.len().div_ceil(16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    true
}

/// Exact division by 127 for a branch-selected Overlay/HardLight product.
/// At least one factor is at most 127, so `value <= 127 * 255`. Folding the
/// powers of 128 in `value + 1` gives the exact quotient across that finite
/// range; the largest intermediate is 32640 and fits in a 16-bit lane.
#[inline]
fn simd_div127(value: u16x16) -> u16x16 {
    const ONE: u16x16 = u16x16::splat(1);
    let value = value + ONE;
    (value + (value >> 7u32) + (value >> 14u32)) >> 7u32
}

#[inline]
fn native_chops_lut_vector(left: [u8; 16], right: [u8; 16], hard_light: bool) -> [u8; 16] {
    const MAX: u16x16 = u16x16::splat(255);
    const SPLIT: u16x16 = u16x16::splat(128);
    let left = u16x16::from(u8x16::new(left));
    let right = u16x16::from(u8x16::new(right));
    let low_condition = if hard_light {
        right.simd_lt(SPLIT)
    } else {
        left.simd_lt(SPLIT)
    };
    let product = low_condition.select(left, MAX - left) * low_condition.select(right, MAX - right);
    let quotient = simd_div127(product);
    simd_pack_u16x16(low_condition.select(quotient, MAX - quotient)).to_array()
}

/// Apply Pillow's exact 256×256 Overlay/HardLight LUT formula to native
/// bytes.  The operation is independent for each stored sample, so the
/// vector stream may cross image-row boundaries. A padded final block keeps
/// short indexed images on the real SIMD data path without writing padding
/// lanes into the result.
fn native_chops_lut_formula(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    hard_light: bool,
) -> Option<DynamicImage> {
    let channels = native_chops_pair_channels(img, other, mode)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    if left.len() != right.len() || !left.len().is_multiple_of(channels) {
        return None;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    let mut output = vec![0u8; left.len()];
    let (left_blocks, left_tail) = left.as_chunks::<16>();
    let (right_blocks, right_tail) = right.as_chunks::<16>();
    let (output_blocks, output_tail) = output.as_chunks_mut::<16>();
    for ((left, right), output) in left_blocks.iter().zip(right_blocks).zip(output_blocks) {
        *output = native_chops_lut_vector(*left, *right, hard_light);
    }
    if !output_tail.is_empty() {
        let active = output_tail.len();
        let mut left_padded = [0u8; 16];
        let mut right_padded = [0u8; 16];
        left_padded[..active].copy_from_slice(left_tail);
        right_padded[..active].copy_from_slice(right_tail);
        let values = native_chops_lut_vector(left_padded, right_padded, hard_light);
        output_tail.copy_from_slice(&values[..active]);
    }
    crate::compute::record_pipeline_operation_vector_blocks(output.len().div_ceil(16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result))
}

pub(super) fn native_chops_overlay(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    native_chops_lut_formula(img, other, mode, false)
}

pub(super) fn native_chops_hard_light(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    native_chops_lut_formula(img, other, mode, true)
}

/// Evaluate sixteen exact Pillow SoftLight samples with integer SIMD lanes.
///
/// Pillow's CHOP2 implementation is:
/// `((255-a)*a*b)/65536 + (a*(255-((255-a)*(255-b)/255)))/255`.
/// The intermediate divisions are truncating integer divisions, so preserve
/// their order. The divide-by-255 products fit in 16 bits; decompose the
/// divide-by-65536 product into high and low bytes to keep it exact in 16 bits.
#[inline]
fn native_chops_soft_light_vector(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    const MAX: u16x16 = u16x16::splat(255);
    let a = u16x16::from(u8x16::new(left));
    let b = u16x16::from(u8x16::new(right));
    let inverse_a = MAX - a;

    let inverse_product = inverse_a * (MAX - b);
    let divided_inverse = simd_div255_u16x16(inverse_product);
    let term2 = simd_div255_u16x16(a * (MAX - divided_inverse));

    let complement_product = inverse_a * a;
    let high = (complement_product >> 8u32) * b;
    let low = (complement_product & MAX) * b;
    let term1 = (high + (low >> 8u32)) >> 8u32;
    let result = term1 + term2;
    simd_pack_u16x16(result.simd_gt(MAX).select(MAX, result)).to_array()
}

/// Apply SoftLight directly to native interleaved bytes. Chops treats alpha
/// as an ordinary stored sample, so LA/RGBA and CMYK all use the same active
/// channel stream. Row boundaries are not semantic boundaries for this
/// independent per-byte formula and may be crossed by vector blocks. The
/// final block is padded and only its active lanes are retained.
pub(super) fn native_chops_soft_light(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
) -> Option<DynamicImage> {
    let channels = native_chops_pair_channels(img, other, mode)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    if left.len() != right.len() || !left.len().is_multiple_of(channels) {
        return None;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    let mut output = vec![0u8; left.len()];
    let apply = |left: &[u8], right: &[u8], output: &mut [u8]| {
        let (left_blocks, left_tail) = left.as_chunks::<16>();
        let (right_blocks, right_tail) = right.as_chunks::<16>();
        let (output_blocks, output_tail) = output.as_chunks_mut::<16>();
        for ((left, right), output) in left_blocks.iter().zip(right_blocks).zip(output_blocks) {
            *output = native_chops_soft_light_vector(*left, *right);
        }
        if !output_tail.is_empty() {
            let active = output_tail.len();
            let mut left_padded = [0u8; 16];
            let mut right_padded = [0u8; 16];
            left_padded[..active].copy_from_slice(left_tail);
            right_padded[..active].copy_from_slice(right_tail);
            let values = native_chops_soft_light_vector(left_padded, right_padded);
            output_tail.copy_from_slice(&values[..active]);
        }
    };
    #[cfg(feature = "parallel")]
    if left.len() >= 1024 * 1024 {
        const TILE_BYTES: usize = 64 * 1024;
        let tiles = left.len().div_ceil(TILE_BYTES);
        simd_rows_mut_serial!(
            &mut output,
            TILE_BYTES,
            tiles,
            |start, _end, _tile, bytes| {
                let end = start + bytes.len();
                apply(&left[start..end], &right[start..end], bytes);
            }
        );
    } else {
        apply(left, right, &mut output);
    }
    #[cfg(not(feature = "parallel"))]
    apply(left, right, &mut output);
    crate::compute::record_pipeline_operation_vector_blocks(output.len().div_ceil(16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
        .ok()
        .map(|result| preserve_mode(img, result))
}

#[inline]
fn native_chops_affine_byte(value: f32) -> u8 {
    if value <= 0.0 {
        0
    } else if value >= 255.0 {
        255
    } else {
        value as u8
    }
}

#[inline]
fn native_chops_affine_vector(
    left: [u8; 8],
    right: [u8; 8],
    scale: f32,
    offset: f32,
    subtract: bool,
) -> [u8; 8] {
    let left = f32x8::new(left.map(f32::from));
    let right = f32x8::new(right.map(f32::from));
    let value = if subtract {
        (left - right) / f32x8::splat(scale) + f32x8::splat(offset)
    } else {
        (left + right) / f32x8::splat(scale) + f32x8::splat(offset)
    };
    value.to_array().map(native_chops_affine_byte)
}

pub(super) fn native_chops_pair_channels(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
) -> Option<usize> {
    let channels = native_chops_layout(img, mode)?;
    (native_chops_layout(other, mode) == Some(channels) && img.dimensions() == other.dimensions())
        .then_some(channels)
}

/// Apply Pillow's scaled/offset Chops formula to native byte samples.
///
/// The arithmetic is vectorized in eight `f32` lanes because Pillow
/// narrows scale to float and evaluates the division and offset addition before
/// clamping and truncating to a byte. Loads are native interleaved bytes; no
/// packed RGBA conversion is introduced for RGB, LA, RGBA, CMYK, HSV, YCbCr,
/// RGBa, or RGBX storage.
pub(super) fn native_chops_affine(
    img: &DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    scale: f64,
    offset: f64,
    subtract: bool,
) -> Option<DynamicImage> {
    let channels = native_chops_pair_channels(img, other, mode)?;
    if !scale.is_finite() || !offset.is_finite() {
        return None;
    }
    let scale = scale as f32;
    let offset = offset as f32;
    let width = img.width() as usize;
    let height = img.height() as usize;
    let row_stride = width.checked_mul(channels)?;
    let left = img.as_bytes();
    let right = other.as_bytes();
    if row_stride.checked_mul(height) != Some(left.len()) || right.len() != left.len() {
        return None;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Some(img.clone());
    }
    if row_stride < 8 {
        return None;
    }

    let mut output = vec![0u8; left.len()];
    let vector_blocks = (row_stride / 8).saturating_mul(height);
    let scalar_tail = (row_stride % 8).saturating_mul(height);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);

    for row in 0..height {
        let row_start = row * row_stride;
        let left_row = &left[row_start..row_start + row_stride];
        let right_row = &right[row_start..row_start + row_stride];
        let output_row = &mut output[row_start..row_start + row_stride];
        let vector_len = output_row.len() / 8 * 8;
        for start in (0..vector_len).step_by(8) {
            let left_block = <[u8; 8]>::try_from(&left_row[start..start + 8]).ok()?;
            let right_block = <[u8; 8]>::try_from(&right_row[start..start + 8]).ok()?;
            output_row[start..start + 8].copy_from_slice(&native_chops_affine_vector(
                left_block,
                right_block,
                scale,
                offset,
                subtract,
            ));
        }
        for index in vector_len..output_row.len() {
            let value = if subtract {
                (f32::from(left_row[index]) - f32::from(right_row[index])) / scale + offset
            } else {
                (f32::from(left_row[index]) + f32::from(right_row[index])) / scale + offset
            };
            output_row[index] = native_chops_affine_byte(value);
        }
    }
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), output, channels)
            .ok()
            .map(|result| preserve_mode(img, result));
    if result.is_some() {
        crate::compute::record_pipeline_operation_path("vector");
    }
    result
}

/// In-place version of [`native_chops_affine`] for an owned SIMD intermediate.
/// The right operand is materialized once and remains immutable while the
/// left buffer is updated after each vector block has been loaded.
pub(super) fn native_chops_affine_in_place(
    img: &mut DynamicImage,
    other: &DynamicImage,
    mode: Option<&str>,
    scale: f64,
    offset: f64,
    subtract: bool,
) -> bool {
    let Some(channels) = native_chops_pair_channels(&*img, other, mode) else {
        return false;
    };
    if !scale.is_finite() || !offset.is_finite() {
        return false;
    }
    let scale = scale as f32;
    let offset = offset as f32;
    let width = img.width() as usize;
    let height = img.height() as usize;
    let Some(row_stride) = width.checked_mul(channels) else {
        return false;
    };
    let right = other.as_bytes();
    let Some(left) = img.as_bytes_mut() else {
        return false;
    };
    if row_stride.checked_mul(height) != Some(left.len()) || right.len() != left.len() {
        return false;
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return true;
    }
    if row_stride < 8 {
        return false;
    }

    let vector_blocks = (row_stride / 8).saturating_mul(height);
    let scalar_tail = (row_stride % 8).saturating_mul(height);
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    for row in 0..height {
        let row_start = row * row_stride;
        let left_row = &mut left[row_start..row_start + row_stride];
        let right_row = &right[row_start..row_start + row_stride];
        let vector_len = left_row.len() / 8 * 8;
        for start in (0..vector_len).step_by(8) {
            let left_block = <[u8; 8]>::try_from(&left_row[start..start + 8])
                .expect("native affine Chops block has 8 bytes");
            let right_block = <[u8; 8]>::try_from(&right_row[start..start + 8])
                .expect("native affine Chops block has 8 bytes");
            left_row[start..start + 8].copy_from_slice(&native_chops_affine_vector(
                left_block,
                right_block,
                scale,
                offset,
                subtract,
            ));
        }
        for index in vector_len..left_row.len() {
            let value = if subtract {
                (f32::from(left_row[index]) - f32::from(right_row[index])) / scale + offset
            } else {
                (f32::from(left_row[index]) + f32::from(right_row[index])) / scale + offset
            };
            left_row[index] = native_chops_affine_byte(value);
        }
    }
    crate::compute::record_pipeline_operation_path("vector");
    true
}

macro_rules! native_bytewise_chops {
    ($name:ident, $vector_op:expr, $scalar_op:expr) => {
        pub(super) fn $name(
            img: &DynamicImage,
            other: &DynamicImage,
            mode: Option<&str>,
        ) -> Option<DynamicImage> {
            native_chops_bytewise(img, other, mode, $vector_op, $scalar_op)
        }
    };
}

native_bytewise_chops!(
    native_chops_darker,
    |left: u8x16, right: u8x16| left.min(right),
    |left: u8, right: u8| left.min(right)
);
native_bytewise_chops!(
    native_chops_lighter,
    |left: u8x16, right: u8x16| left.max(right),
    |left: u8, right: u8| left.max(right)
);
native_bytewise_chops!(
    native_chops_difference,
    |left: u8x16, right: u8x16| left.max(right) - left.min(right),
    |left: u8, right: u8| left.abs_diff(right)
);
native_bytewise_chops!(
    native_chops_add_modulo,
    |left: u8x16, right: u8x16| left + right,
    |left: u8, right: u8| left.wrapping_add(right)
);
native_bytewise_chops!(
    native_chops_subtract_modulo,
    |left: u8x16, right: u8x16| left - right,
    |left: u8, right: u8| left.wrapping_sub(right)
);
native_bytewise_chops!(
    native_chops_add_clamped,
    |left: u8x16, right: u8x16| left.saturating_add(right),
    |left: u8, right: u8| left.saturating_add(right)
);
native_bytewise_chops!(
    native_chops_subtract_clamped,
    |left: u8x16, right: u8x16| left.saturating_sub(right),
    |left: u8, right: u8| left.saturating_sub(right)
);
// A mode-1 sample need not be 0/255. The minimum is nonzero exactly when both
// unsigned operands are nonzero; the comparison emits canonical result bytes.
#[inline]
pub(super) fn native_logical_and_vector(left: u8x16, right: u8x16) -> u8x16 {
    left.min(right)
        .simd_eq(u8x16::ZERO)
        .select(u8x16::ZERO, u8x16::splat(255))
}

native_bytewise_chops!(
    native_chops_logical_and,
    native_logical_and_vector,
    |left: u8, right: u8| u8::from(left != 0 && right != 0) * 255
);
#[inline]
pub(super) fn native_logical_or_vector(left: u8x16, right: u8x16) -> u8x16 {
    (left | right)
        .simd_eq(u8x16::ZERO)
        .select(u8x16::ZERO, u8x16::splat(255))
}

native_bytewise_chops!(
    native_chops_logical_or,
    native_logical_or_vector,
    |left: u8, right: u8| u8::from((left | right) != 0) * 255
);
#[inline]
pub(super) fn native_logical_xor_vector(left: u8x16, right: u8x16) -> u8x16 {
    // Complementing both boolean operands leaves XOR unchanged. Comparing
    // with zero directly supplies canonical byte masks without inversions.
    left.simd_eq(u8x16::ZERO)
        .select(u8x16::splat(255), u8x16::ZERO)
        ^ right
            .simd_eq(u8x16::ZERO)
            .select(u8x16::splat(255), u8x16::ZERO)
}

native_bytewise_chops!(
    native_chops_logical_xor,
    native_logical_xor_vector,
    |left: u8, right: u8| u8::from((left != 0) ^ (right != 0)) * 255
);

mod fused_multiply_screen;
pub(super) use self::fused_multiply_screen::simd_fused_multiply_screen_row;

#[cfg(test)]
mod tests {
    use super::{native_chops_lut_vector, simd_div127};

    #[test]
    fn div127_matches_every_selected_blend_product() {
        for start in (0..=32385u16).step_by(16) {
            let values = std::array::from_fn(|lane| (start + lane as u16).min(32385));
            let actual = simd_div127(wide::u16x16::new(values)).to_array();
            assert_eq!(actual, values.map(|value| value / 127));
        }
    }

    #[test]
    fn overlay_and_hardlight_vectors_match_all_byte_pairs() {
        let tables: [(bool, &[u8; 65536]); 2] = [
            (false, include_bytes!("../../../../../ops/lut_overlay.bin")),
            (true, include_bytes!("../../../../../ops/lut_hardlight.bin")),
        ];
        for (hard_light, table) in tables {
            for start in (0..65536usize).step_by(16) {
                let left = std::array::from_fn(|lane| ((start + lane) >> 8) as u8);
                let right = std::array::from_fn(|lane| (start + lane) as u8);
                assert_eq!(
                    native_chops_lut_vector(left, right, hard_light),
                    table[start..start + 16],
                    "hard_light={hard_light} pair_start={start}"
                );
            }
        }
    }
}
