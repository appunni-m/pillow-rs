//! Shared portable-vector byte kernel used by declared CPU ISA strategies.

use crate::error::PilError;
use wide::u8x16;

pub(crate) use crate::compute::pool_cpu::helpers::bytes::native_invert::{
    NativeInvertOutput,
    invert,
};

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

