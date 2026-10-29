//! Original vector allocation/traversal contracts, with layout and ISA already selected.

use crate::error::PilError;
use crate::raster::{DynamicImage, Pixel};
use super::extract_band::{CheckedBand, vector_output};

#[cfg(target_arch = "x86_64")]
#[allow(unsafe_code)]
/// # Safety
///
/// The prepared owner must establish SSE2/SSSE3 support, CHANNELS in 2..=4,
/// and exact nonempty native storage with an index inside that layout.
pub(crate) unsafe fn ssse3<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    input: CheckedBand<'_, P>,
) -> Result<DynamicImage, PilError> {
    use crate::compute::pool_cpu::helpers::channel::ssse3 as kernels;
    let source = input.input.as_raw();
    let channel = input.index.min(CHANNELS - 1);
    let output = if CHANNELS == 3 {
        // SAFETY: the prepared owner established the documented instruction,
        // layout, index, and exact source-length preconditions.
        let output = unsafe {
            kernels::gather_interleaved_channel_ssse3(source, CHANNELS, channel)
        };
        crate::compute::record_pipeline_allocation(output.0.len());
        output
    } else {
        let mut output = input.dimensions.alloc_buffer();
        // SAFETY: the source contains exactly width*height*CHANNELS bytes;
        // the separately allocated output has width*height writable bytes.
        // Selection established the channel index and SSE2/SSSE3 capability.
        let (blocks, tail) = unsafe {
            kernels::gather_interleaved_channel_ssse3_into_raw(
                source, output.as_mut_ptr(), CHANNELS, channel,
            )
        };
        (output, blocks, tail)
    };
    vector_output(input.input.width(), input.input.height(), output, "ssse3-gather")
}

#[cfg(any(
    all(target_arch = "x86", any(target_feature = "sse2", target_feature = "ssse3")),
    all(target_arch = "wasm32", target_feature = "simd128"),
))]
fn gather<const CHANNELS: usize>(source: &[u8], channel: usize) -> (Vec<u8>, u64, u64) {
    use crate::compute::pool_cpu::helpers::channel::wide;
    match channel {
        0 => wide::gather::<CHANNELS, 0>(source),
        1 => wide::gather::<CHANNELS, 1>(source),
        2 => wide::gather::<CHANNELS, 2>(source),
        _ => wide::gather::<CHANNELS, 3>(source),
    }
}

#[cfg(any(
    all(target_arch = "x86", any(target_feature = "sse2", target_feature = "ssse3")),
    all(target_arch = "wasm32", target_feature = "simd128"),
))]
pub(crate) fn portable<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    input: CheckedBand<'_, P>,
) -> Result<DynamicImage, PilError> {
    let channel = input.index.min(CHANNELS - 1);
    let output = if CHANNELS == 3 {
        let output = gather::<CHANNELS>(input.input.as_raw(), channel);
        crate::compute::record_pipeline_allocation(output.0.len());
        output
    } else {
        let mut output = input.dimensions.alloc_buffer();
        let (gathered, blocks, tail) = gather::<CHANNELS>(input.input.as_raw(), channel);
        output.copy_from_slice(&gathered);
        (output, blocks, tail)
    };
    vector_output(input.input.width(), input.input.height(), output, "vector-gather")
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) struct NeonBand<'a, P: Pixel> {
    input: CheckedBand<'a, P>,
    neon: fearless_simd::Neon,
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) fn prepare_neon<P: Pixel>(input: CheckedBand<'_, P>) -> NeonBand<'_, P> {
    // This module requires a statically guaranteed NEON target; the operation
    // also selected NEON through the injected context. No leaf probe is needed.
    NeonBand { input, neon: fearless_simd::Neon::assume_supported() }
}

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) fn neon<P: Pixel<Subpixel = u8>, const CHANNELS: usize>(
    kernel: NeonBand<'_, P>,
) -> Result<DynamicImage, PilError> {
    use crate::compute::pool_cpu::helpers::channel::neon;
    let input = kernel.input;
    let channel = input.index.min(CHANNELS - 1);
    let mut output = if CHANNELS == 3 {
        vec![0; input.input.as_raw().len() / CHANNELS]
    } else {
        input.dimensions.alloc_buffer()
    };
    let (blocks, tail) = neon::extract_channel_into::<CHANNELS>(
        input.input.as_raw(), &mut output, channel, kernel.neon,
    );
    if CHANNELS == 3 {
        crate::compute::record_pipeline_allocation(output.len());
    }
    vector_output(input.input.width(), input.input.height(),
        (output, blocks, tail), "neon-deinterleave")
}
