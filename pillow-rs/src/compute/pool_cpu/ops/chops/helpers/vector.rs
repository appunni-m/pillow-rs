//! Portable native-byte vectors for the exact-mode Chops CPU strategies.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::ImageBuffer;
use crate::raster::traits::Pixel;
use wide::{f32x8, u8x16, u16x16};

use super::binary::BinaryOperation;

/// Executes a validated native-byte architecture strategy.
pub(crate) fn execute<P>(
    image: &ImageBuffer<P, Vec<u8>>,
    request: super::super::ops::ChopsRequest,
    other: Option<&ImageBuffer<P, Vec<u8>>>,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let operation = match request {
        super::super::ops::ChopsRequest::Offset { .. } => {
            return Err(PilError::InternalError(
                "Chops Offset must be dispatched by its exact-mode strategy".into(),
            ));
        }
        super::super::ops::ChopsRequest::Invert => return execute_invert(image),
        super::super::ops::ChopsRequest::Binary(operation) => operation,
        _ => {
            return Err(PilError::InternalError(
                "Chops vector strategy received a non-binary request".into(),
            ));
        }
    };
    let other = other.ok_or_else(|| {
        PilError::InternalError("Chops vector plan lost its secondary image".into())
    })?;
    match operation {
        operation @ (BinaryOperation::Add { .. } | BinaryOperation::Subtract { .. }) => {
            execute_native(image, other, operation)
        }
        BinaryOperation::Multiply => execute_bytes(image, other, multiply_block),
        BinaryOperation::Screen => execute_bytes(image, other, screen_block),
        BinaryOperation::Darker => execute_bytes(image, other, darker_block),
        BinaryOperation::Lighter => execute_bytes(image, other, lighter_block),
        BinaryOperation::Difference => execute_bytes(image, other, difference_block),
        BinaryOperation::Overlay => execute_bytes(image, other, overlay_block),
        BinaryOperation::HardLight => execute_bytes(image, other, hard_light_block),
        BinaryOperation::SoftLight => execute_bytes(image, other, soft_light_block),
        BinaryOperation::AddModulo => execute_bytes(image, other, add_modulo_block),
        BinaryOperation::SubtractModulo => execute_bytes(image, other, subtract_modulo_block),
        BinaryOperation::LogicalAnd => execute_bytes(image, other, logical_and_block),
        BinaryOperation::LogicalOr => execute_bytes(image, other, logical_or_block),
        BinaryOperation::LogicalXor => execute_bytes(image, other, logical_xor_block),
    }
}

fn execute_invert<P>(image: &ImageBuffer<P, Vec<u8>>) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let channels = P::CHANNEL_COUNT;
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), channels)?;
    let source = image.as_raw().as_slice();
    if source.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "Chops invert source did not match its admitted native layout".into(),
        ));
    }
    match crate::compute::pool_cpu::helpers::bytes::native_invert::invert(
        source,
        image.width(),
        image.height(),
        usize::from(channels),
        true,
    )? {
        crate::compute::pool_cpu::helpers::bytes::native_invert::NativeInvertOutput::Empty => {
            crate::compute::record_pipeline_operation_path("native-copy");
            Ok(image.clone())
        }
        crate::compute::pool_cpu::helpers::bytes::native_invert::NativeInvertOutput::Exact(
            bytes,
        ) => {
            crate::compute::record_pipeline_operation_path("vector");
            let mut output =
                ImageBuffer::<P, Vec<u8>>::from_raw(image.width(), image.height(), bytes)
                    .ok_or_else(|| {
                        PilError::InternalError("invert Chops buffer shape mismatch".into())
                    })?;
            super::unary::carry_inverted_hidden_lanes(image, &mut output)?;
            Ok(output)
        }
        crate::compute::pool_cpu::helpers::bytes::native_invert::NativeInvertOutput::Prefix(_) => {
            Err(PilError::InternalError(
                "Chops invert kernel returned trailing native bytes".into(),
            ))
        }
    }
}

fn execute_bytes<P, K>(
    image: &ImageBuffer<P, Vec<u8>>,
    other: &ImageBuffer<P, Vec<u8>>,
    kernel: K,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
    K: Fn([u8; 16], [u8; 16]) -> [u8; 16],
{
    let channels = P::CHANNEL_COUNT;
    if image.dimensions() != other.dimensions() {
        return Err(PilError::InternalError(
            "Chops vector operands did not match their admitted native layout".into(),
        ));
    }
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), channels)?;
    let left = image.as_raw().as_slice();
    let right = other.as_raw().as_slice();
    if left.len() != dimensions.total_bytes() || right.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "Chops vector operands did not match their image dimensions".into(),
        ));
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(image.clone());
    }
    let mut output = dimensions.alloc_buffer();
    apply_blocks(left, right, &mut output, kernel);
    crate::compute::record_pipeline_operation_vector_blocks(
        u64::try_from(left.len().div_ceil(16)).unwrap_or(u64::MAX),
    );
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    ImageBuffer::<P, Vec<u8>>::from_raw(image.width(), image.height(), output)
        .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))
}

#[inline(always)]
fn apply_blocks<K>(left: &[u8], right: &[u8], output: &mut [u8], kernel: K)
where
    K: Fn([u8; 16], [u8; 16]) -> [u8; 16],
{
    let (left_blocks, left_tail) = left.as_chunks::<16>();
    let (right_blocks, right_tail) = right.as_chunks::<16>();
    let (output_blocks, output_tail) = output.as_chunks_mut::<16>();
    for ((left, right), output) in left_blocks.iter().zip(right_blocks).zip(output_blocks) {
        *output = kernel(*left, *right);
    }
    if !output_tail.is_empty() {
        let active = output_tail.len();
        let mut left_padded = [0; 16];
        let mut right_padded = [0; 16];
        left_padded[..active].copy_from_slice(left_tail);
        right_padded[..active].copy_from_slice(right_tail);
        output_tail.copy_from_slice(&kernel(left_padded, right_padded)[..active]);
    }
}

fn execute_native<P>(
    image: &ImageBuffer<P, Vec<u8>>,
    other: &ImageBuffer<P, Vec<u8>>,
    operation: BinaryOperation,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let channels = P::CHANNEL_COUNT;
    if image.dimensions() != other.dimensions() {
        return Err(PilError::InternalError(
            "Chops vector operands did not match their admitted native layout".into(),
        ));
    }
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), channels)?;
    let left = image.as_raw().as_slice();
    let right = other.as_raw().as_slice();
    if left.len() != dimensions.total_bytes() || right.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "Chops vector operands did not match their image dimensions".into(),
        ));
    }
    let (scale, offset, subtract) = affine_parameters(operation)?;
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(image.clone());
    }
    let mut output = dimensions.alloc_buffer();
    if exact_default_parameters(operation) {
        apply_saturating(left, right, &mut output, subtract);
        crate::compute::record_pipeline_operation_vector_blocks(
            u64::try_from(left.len().div_ceil(16)).unwrap_or(u64::MAX),
        );
        crate::compute::record_pipeline_operation_scalar_tail(0);
    } else {
        let (blocks, scalar_tail) = apply_affine(
            left,
            right,
            &mut output,
            dimensions.row_stride(),
            image.height() as usize,
            scale,
            offset,
            subtract,
        );
        crate::compute::record_pipeline_operation_vector_blocks(
            u64::try_from(blocks).unwrap_or(u64::MAX),
        );
        crate::compute::record_pipeline_operation_scalar_tail(
            u64::try_from(scalar_tail).unwrap_or(u64::MAX),
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    ImageBuffer::<P, Vec<u8>>::from_raw(image.width(), image.height(), output)
        .ok_or_else(|| PilError::ValueError("channel_op_binary buffer error".into()))
}

/// Applies an admitted Add/Sub vector strategy to an owned batch intermediate.
pub(crate) fn execute_in_place<P>(
    image: &mut ImageBuffer<P, Vec<u8>>,
    request: super::super::ops::ChopsRequest,
    other: &ImageBuffer<P, Vec<u8>>,
) -> Result<(), PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let super::super::ops::ChopsRequest::Binary(operation) = request else {
        return Err(PilError::InternalError(
            "Chops in-place vector strategy received a non-binary request".into(),
        ));
    };
    let (scale, offset, subtract) = affine_parameters(operation)?;
    let channels = P::CHANNEL_COUNT;
    if image.dimensions() != other.dimensions() {
        return Err(PilError::InternalError(
            "Chops vector operands did not match their admitted native layout".into(),
        ));
    }
    let dimensions = CheckedDims::new_allow_empty(image.width(), image.height(), channels)?;
    let height = image.height() as usize;
    let right = other.as_raw().as_slice();
    let left: &mut [u8] = image;
    if left.len() != dimensions.total_bytes() || right.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "Chops vector operands did not match their image dimensions".into(),
        ));
    }
    if left.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(());
    }
    let row_stride = dimensions.row_stride();
    if exact_default_parameters(operation) && row_stride >= 16 {
        apply_saturating_in_place(left, right, subtract);
        crate::compute::record_pipeline_operation_vector_blocks(
            u64::try_from(left.len().div_ceil(16)).unwrap_or(u64::MAX),
        );
        crate::compute::record_pipeline_operation_scalar_tail(0);
    } else {
        let (blocks, scalar_tail) =
            apply_affine_in_place(left, right, row_stride, height, scale, offset, subtract);
        crate::compute::record_pipeline_operation_vector_blocks(
            u64::try_from(blocks).unwrap_or(u64::MAX),
        );
        crate::compute::record_pipeline_operation_scalar_tail(
            u64::try_from(scalar_tail).unwrap_or(u64::MAX),
        );
    }
    crate::compute::record_pipeline_operation_path("vector");
    Ok(())
}

fn affine_parameters(operation: BinaryOperation) -> Result<(f32, f32, bool), PilError> {
    match operation {
        BinaryOperation::Add { scale, offset, .. } => Ok((scale, offset, false)),
        BinaryOperation::Subtract { scale, offset, .. } => Ok((scale, offset, true)),
        _ => Err(PilError::InternalError(
            "Chops vector strategy received a non-Add/Sub operation".into(),
        )),
    }
}

fn exact_default_parameters(operation: BinaryOperation) -> bool {
    match operation {
        BinaryOperation::Add {
            exact_default_parameters,
            ..
        }
        | BinaryOperation::Subtract {
            exact_default_parameters,
            ..
        } => exact_default_parameters,
        _ => false,
    }
}

fn apply_saturating(left: &[u8], right: &[u8], output: &mut [u8], subtract: bool) {
    let (left_blocks, left_tail) = left.as_chunks::<16>();
    let (right_blocks, right_tail) = right.as_chunks::<16>();
    let (output_blocks, output_tail) = output.as_chunks_mut::<16>();
    for ((left, right), output) in left_blocks.iter().zip(right_blocks).zip(output_blocks) {
        let left = u8x16::new(*left);
        let right = u8x16::new(*right);
        *output = if subtract {
            left.saturating_sub(right).to_array()
        } else {
            left.saturating_add(right).to_array()
        };
    }
    if !output_tail.is_empty() {
        let active = output_tail.len();
        let mut left_padded = [0; 16];
        let mut right_padded = [0; 16];
        left_padded[..active].copy_from_slice(left_tail);
        right_padded[..active].copy_from_slice(right_tail);
        let left = u8x16::new(left_padded);
        let right = u8x16::new(right_padded);
        let values = if subtract {
            left.saturating_sub(right)
        } else {
            left.saturating_add(right)
        };
        output_tail.copy_from_slice(&values.to_array()[..active]);
    }
}

fn apply_saturating_in_place(left: &mut [u8], right: &[u8], subtract: bool) {
    let (left_blocks, left_tail) = left.as_chunks_mut::<16>();
    let (right_blocks, right_tail) = right.as_chunks::<16>();
    for (left_block, right_block) in left_blocks.iter_mut().zip(right_blocks) {
        let left_value = u8x16::new(*left_block);
        let right_value = u8x16::new(*right_block);
        *left_block = if subtract {
            left_value.saturating_sub(right_value).to_array()
        } else {
            left_value.saturating_add(right_value).to_array()
        };
    }
    if !left_tail.is_empty() {
        let active = left_tail.len();
        let mut left_padded = [0; 16];
        let mut right_padded = [0; 16];
        left_padded[..active].copy_from_slice(left_tail);
        right_padded[..active].copy_from_slice(right_tail);
        let left = u8x16::new(left_padded);
        let right = u8x16::new(right_padded);
        let output = if subtract {
            left.saturating_sub(right)
        } else {
            left.saturating_add(right)
        };
        left_tail.copy_from_slice(&output.to_array()[..active]);
    }
}

fn apply_affine_in_place(
    left: &mut [u8],
    right: &[u8],
    row_stride: usize,
    height: usize,
    scale: f32,
    offset: f32,
    subtract: bool,
) -> (usize, usize) {
    let vector_bytes = row_stride / 8 * 8;
    for row in 0..height {
        let start = row * row_stride;
        let left_row = &mut left[start..start + row_stride];
        let right_row = &right[start..start + row_stride];
        for start in (0..vector_bytes).step_by(8) {
            let left_block = <[u8; 8]>::try_from(&left_row[start..start + 8])
                .expect("complete affine vector block");
            let right_block = <[u8; 8]>::try_from(&right_row[start..start + 8])
                .expect("complete affine vector block");
            left_row[start..start + 8].copy_from_slice(&affine_block(
                left_block,
                right_block,
                scale,
                offset,
                subtract,
            ));
        }
        for index in vector_bytes..row_stride {
            left_row[index] =
                affine_sample(left_row[index], right_row[index], scale, offset, subtract);
        }
    }
    (
        vector_bytes / 8 * height,
        (row_stride - vector_bytes) * height,
    )
}

fn apply_affine(
    left: &[u8],
    right: &[u8],
    output: &mut [u8],
    row_stride: usize,
    height: usize,
    scale: f32,
    offset: f32,
    subtract: bool,
) -> (usize, usize) {
    let vector_bytes = row_stride / 8 * 8;
    for row in 0..height {
        let start = row * row_stride;
        let left_row = &left[start..start + row_stride];
        let right_row = &right[start..start + row_stride];
        let output_row = &mut output[start..start + row_stride];
        for start in (0..vector_bytes).step_by(8) {
            let left_block = <[u8; 8]>::try_from(&left_row[start..start + 8])
                .expect("complete affine vector block");
            let right_block = <[u8; 8]>::try_from(&right_row[start..start + 8])
                .expect("complete affine vector block");
            output_row[start..start + 8].copy_from_slice(&affine_block(
                left_block,
                right_block,
                scale,
                offset,
                subtract,
            ));
        }
        for index in vector_bytes..row_stride {
            output_row[index] =
                affine_sample(left_row[index], right_row[index], scale, offset, subtract);
        }
    }
    (
        vector_bytes / 8 * height,
        (row_stride - vector_bytes) * height,
    )
}

#[inline(always)]
fn affine_block(left: [u8; 8], right: [u8; 8], scale: f32, offset: f32, subtract: bool) -> [u8; 8] {
    let left = f32x8::new(left.map(f32::from));
    let right = f32x8::new(right.map(f32::from));
    let values = if subtract { left - right } else { left + right };
    (values / f32x8::splat(scale) + f32x8::splat(offset))
        .to_array()
        .map(|value| value.clamp(0.0, 255.0) as u8)
}

#[inline]
fn affine_sample(left: u8, right: u8, scale: f32, offset: f32, subtract: bool) -> u8 {
    let left = f32::from(left);
    let right = f32::from(right);
    let value = if subtract { left - right } else { left + right };
    (value / scale + offset).clamp(0.0, 255.0) as u8
}

#[inline]
pub(super) fn divide_255(value: u16x16) -> u16x16 {
    const ONE: u16x16 = u16x16::splat(1);
    let incremented = value + ONE;
    (incremented + (incremented >> 8u32)) >> 8u32
}

#[inline(always)]
fn multiply_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    let left = u16x16::from(u8x16::new(left));
    let right = u16x16::from(u8x16::new(right));
    crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16(divide_255(left * right))
        .to_array()
}

#[inline(always)]
fn screen_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    const MAX: u16x16 = u16x16::splat(255);
    let left = u16x16::from(u8x16::new(left));
    let right = u16x16::from(u8x16::new(right));
    let value = MAX - divide_255((MAX - left) * (MAX - right));
    crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16(value).to_array()
}

#[inline(always)]
fn darker_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    (u8x16::new(left).min(u8x16::new(right))).to_array()
}

#[inline(always)]
fn lighter_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    (u8x16::new(left).max(u8x16::new(right))).to_array()
}

#[inline(always)]
fn difference_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    let left = u8x16::new(left);
    let right = u8x16::new(right);
    (left.max(right) - left.min(right)).to_array()
}

#[inline(always)]
fn add_modulo_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    (u8x16::new(left) + u8x16::new(right)).to_array()
}

#[inline(always)]
fn subtract_modulo_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    (u8x16::new(left) - u8x16::new(right)).to_array()
}

#[inline]
fn divide_127(value: u16x16) -> u16x16 {
    const ONE: u16x16 = u16x16::splat(1);
    let value = value + ONE;
    (value + (value >> 7u32) + (value >> 14u32)) >> 7u32
}

#[inline]
fn overlay_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    lut_block(left, right, false)
}

#[inline]
fn hard_light_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    lut_block(left, right, true)
}

#[inline(always)]
fn lut_block(left: [u8; 16], right: [u8; 16], hard_light: bool) -> [u8; 16] {
    const MAX: u16x16 = u16x16::splat(255);
    const SPLIT: u16x16 = u16x16::splat(128);
    let left = u16x16::from(u8x16::new(left));
    let right = u16x16::from(u8x16::new(right));
    let low = if hard_light {
        right.simd_lt(SPLIT)
    } else {
        left.simd_lt(SPLIT)
    };
    let product = low.select(left, MAX - left) * low.select(right, MAX - right);
    let quotient = divide_127(product);
    let value = low.select(quotient, MAX - quotient);
    crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16(value).to_array()
}

#[inline(always)]
fn soft_light_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    const MAX: u16x16 = u16x16::splat(255);
    let left = u16x16::from(u8x16::new(left));
    let right = u16x16::from(u8x16::new(right));
    let inverse_left = MAX - left;
    let divided_inverse = divide_255(inverse_left * (MAX - right));
    let term2 = divide_255(left * (MAX - divided_inverse));
    let complement_product = inverse_left * left;
    let high = (complement_product >> 8u32) * right;
    let low = (complement_product & MAX) * right;
    let term1 = (high + (low >> 8u32)) >> 8u32;
    let value = term1 + term2;
    let clamped = value.simd_gt(MAX).select(MAX, value);
    crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16(clamped).to_array()
}

#[inline(always)]
fn logical_and_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    let nonzero = u8x16::new(left).min(u8x16::new(right)).simd_eq(u8x16::ZERO);
    nonzero.select(u8x16::ZERO, u8x16::splat(255)).to_array()
}

#[inline(always)]
fn logical_or_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    let nonzero = (u8x16::new(left) | u8x16::new(right)).simd_eq(u8x16::ZERO);
    nonzero.select(u8x16::ZERO, u8x16::splat(255)).to_array()
}

#[inline(always)]
fn logical_xor_block(left: [u8; 16], right: [u8; 16]) -> [u8; 16] {
    let left = u8x16::new(left)
        .simd_eq(u8x16::ZERO)
        .select(u8x16::splat(255), u8x16::ZERO);
    let right = u8x16::new(right)
        .simd_eq(u8x16::ZERO)
        .select(u8x16::splat(255), u8x16::ZERO);
    (left ^ right).to_array()
}
