//! Shared portable-vector byte kernel used by declared CPU ISA strategies.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use wide::u8x16;

const ALL: [u8; 16] = [u8::MAX; 16];
const LA: [u8; 16] = [u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0,
    u8::MAX, 0, u8::MAX, 0, u8::MAX, 0, u8::MAX, 0];
const RGBA: [u8; 16] = [u8::MAX, u8::MAX, u8::MAX, 0, u8::MAX, u8::MAX, u8::MAX, 0,
    u8::MAX, u8::MAX, u8::MAX, 0, u8::MAX, u8::MAX, u8::MAX, 0];

pub(crate) enum NativeInvertOutput {
    Empty,
    Exact(Vec<u8>),
    Prefix(Vec<u8>),
}

/// Byte-wise point transform selected by the exact operation strategy.
#[derive(Clone, Copy)]
pub(crate) enum NativeBytePointTransform {
    Posterize { bits: u8 },
    Solarize { threshold: u8 },
}

/// Applies a native byte point transform with the portable vector lane type.
///
/// The final partial block is padded and transformed as one vector, matching
/// the archived Posterize and Solarize kernels' tail behavior.
pub(crate) fn pointwise(
    source: &[u8],
    operation: NativeBytePointTransform,
) -> Result<Vec<u8>, PilError> {
    let output = match operation {
        NativeBytePointTransform::Posterize { bits } => {
            if !(1..=8).contains(&bits) {
                return Err(PilError::ValueError(
                    "posterize bits must be in range 1..=8".into(),
                ));
            }
            let shift = u32::from(8 - bits);
            let mask = !((1u8 << shift) - 1);
            transform_pointwise_bytes(source, |input| input & u8x16::splat(mask))
        }
        NativeBytePointTransform::Solarize { threshold } => transform_pointwise_bytes(
            source,
            |input| {
                input
                    .simd_ge(u8x16::splat(threshold))
                    .select(u8x16::splat(u8::MAX) - input, input)
            },
        ),
    };
    crate::compute::record_pipeline_operation_vector_blocks(source.len().div_ceil(16) as u64);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(output)
}

fn transform_pointwise_bytes<F>(source: &[u8], transform: F) -> Vec<u8>
where
    F: Fn(u8x16) -> u8x16,
{
    let mut output = Vec::with_capacity(source.len());
    let mut chunks = source.chunks_exact(16);
    for chunk in &mut chunks {
        let block = u8x16::new(
            <[u8; 16]>::try_from(chunk).expect("chunks_exact yields 16-byte blocks"),
        );
        output.extend_from_slice(&transform(block).to_array());
    }
    let remainder = chunks.remainder();
    if !remainder.is_empty() {
        let mut padded = [0; 16];
        padded[..remainder.len()].copy_from_slice(remainder);
        output.extend_from_slice(&transform(u8x16::new(padded)).to_array()[..remainder.len()]);
    }
    output
}

pub(crate) fn invert(
    source: &[u8],
    width: u32,
    height: u32,
    channels: usize,
    invert_alpha: bool,
) -> Result<NativeInvertOutput, PilError> {
    let dims = CheckedDims::new_allow_empty(width, height, channels as u8)?;
    if source.len() < dims.total_bytes() {
        return Err(PilError::InternalError("invert native byte prefix is short".into()));
    }
    if source.is_empty() {
        return Ok(NativeInvertOutput::Empty);
    }
    let mask = active_mask(channels, invert_alpha)?;
    if source.len() == dims.total_bytes() {
        return exact_copy(source, mask, channels).map(NativeInvertOutput::Exact);
    }
    let row_bytes = (width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("invert native row size overflow".into()))?;
    Ok(NativeInvertOutput::Prefix(transform_rows(
        source,
        row_bytes,
        height as usize,
        mask,
    )))
}

fn active_mask(channels: usize, invert_alpha: bool) -> Result<[u8; 16], PilError> {
    match channels {
        1 | 3 => Ok(ALL),
        2 if invert_alpha => Ok(ALL),
        2 => Ok(LA),
        4 if invert_alpha => Ok(ALL),
        4 => Ok(RGBA),
        _ => Err(PilError::InternalError("invert native channel count is invalid".into())),
    }
}

fn exact_copy(source: &[u8], mask: [u8; 16], channels: usize) -> Result<Vec<u8>, PilError> {
    if channels == 1 {
        return invert_single_channel(source);
    }
    let active = u8x16::new(mask);
    let mut output = Vec::with_capacity(source.len());
    let mut chunks = source.chunks_exact(16);
    for chunk in &mut chunks {
        let block = block(chunk)?;
        output.extend_from_slice(&(block ^ active).to_array());
    }
    let remainder = chunks.remainder();
    let mask_offset = source.len() - remainder.len();
    output.extend(remainder.iter().enumerate().map(|(index, value)| {
        *value ^ mask[(mask_offset + index) % 16]
    }));
    crate::compute::record_pipeline_operation_vector_blocks((source.len() / 16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail(remainder.len() as u64);
    Ok(output)
}

fn invert_single_channel(source: &[u8]) -> Result<Vec<u8>, PilError> {
    let full_len = source.len() / 16 * 16;
    let mut output = vec![0; source.len()];
    let active = u8x16::splat(u8::MAX);
    for (input, destination) in source[..full_len]
        .chunks_exact(16)
        .zip(output[..full_len].chunks_exact_mut(16))
    {
        destination.copy_from_slice(&(block(input)? ^ active).to_array());
    }
    for (destination, &input) in output[full_len..].iter_mut().zip(&source[full_len..]) {
        *destination = !input;
    }
    crate::compute::record_pipeline_operation_vector_blocks((full_len / 16) as u64);
    crate::compute::record_pipeline_operation_scalar_tail((source.len() - full_len) as u64);
    Ok(output)
}

fn transform_rows(
    source: &[u8],
    row_bytes: usize,
    height: usize,
    mask: [u8; 16],
) -> Vec<u8> {
    if row_bytes == 0 || height == 0 {
        return source.to_vec();
    }
    let mut output = source.to_vec();
    for row in output.chunks_mut(row_bytes).take(height) {
        transform_row(row, mask);
    }
    let blocks = row_bytes.div_ceil(16).saturating_mul(height);
    crate::compute::record_pipeline_operation_vector_blocks(blocks as u64);
    output
}

fn transform_row(row: &mut [u8], mask: [u8; 16]) {
    let active = u8x16::new(mask);
    let mut chunks = row.chunks_exact_mut(16);
    for chunk in &mut chunks {
        let block = u8x16::new(<[u8; 16]>::try_from(&*chunk)
            .expect("chunks_exact_mut yields 16-byte blocks"));
        chunk.copy_from_slice(&(block ^ active).to_array());
    }
    let remainder = chunks.into_remainder();
    if remainder.is_empty() {
        return;
    }
    let mut padded = [0; 16];
    padded[..remainder.len()].copy_from_slice(remainder);
    let output = (u8x16::new(padded) ^ active).to_array();
    remainder.copy_from_slice(&output[..remainder.len()]);
}

fn block(source: &[u8]) -> Result<u8x16, PilError> {
    let bytes = <[u8; 16]>::try_from(source)
        .map_err(|_| PilError::InternalError("invert SIMD block has invalid width".into()))?;
    Ok(u8x16::new(bytes))
}
