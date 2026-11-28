//! Stream AVX-512F 2x2 I32 reduction blocks into the existing Resize ring.

#![allow(unsafe_code)] // The prepared Thumbnail strategy validates this target feature.

use crate::compute::registry::support::{OperationId, StrategyGuard, StrategySelection};
use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::error::PilError;
use crate::ops::pil_resize::X86FmaToken;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbaImage;

const LANES: usize = crate::compute::pool_cpu::ops::reduce::helpers::i32_avx512f::LANES;

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    context: &ExecutionContext,
    input: &RgbaImage,
    output: (u32, u32),
    filter: ResampleFilter,
    selection: &StrategySelection,
) -> Result<RgbaImage, PilError> {
    let implementation = selection.implementation;
    if context.backend() != Backend::Cpu
        || context.architecture() != Architecture::Avx512F
        || selection.requested_backend != Backend::Cpu
        || selection.requested_architecture != Architecture::Avx512F
        || !context.permits_architecture(Architecture::Avx512F)
        || !context.supports_architecture(Architecture::Avx512F)
        || !context.permits_architecture(Architecture::AvxFma)
        || !context.supports_architecture(Architecture::AvxFma)
        || implementation.backend != Backend::Cpu
        || implementation.operation != OperationId::Thumbnail
        || implementation.mode != crate::raster::Mode::ImageI32
        || implementation.architecture != Architecture::Avx512F
        || selection.guard != StrategyGuard::ThumbnailI32Avx512F
        || input.width() < 32
        || !input.width().is_multiple_of(2)
        || !input.height().is_multiple_of(2)
    {
        return Err(PilError::InternalError(
            "ImageI32 AVX-512F Thumbnail received an ineligible strategy request".into(),
        ));
    }
    let fma = X86FmaToken::from_context(context, Architecture::AvxFma).ok_or_else(|| {
        PilError::InternalError("ImageI32 AVX-512F Thumbnail lacks its resize FMA strategy".into())
    })?;
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let result = super::super::helpers::streaming::execute_with_reducer(
        input,
        output,
        filter,
        &fma,
        |source, source_width, reduced_row, reduced_y| {
            reduce_source_row_pair_avx512f(
                source,
                source_width,
                reduced_row,
                reduced_y,
                &mut vector_blocks,
                &mut scalar_tail,
            );
        },
    )?;
    crate::compute::record_pipeline_operation_path("avx512f");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(result)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
fn reduce_source_row_pair_avx512f(
    source: &[i32],
    source_width: usize,
    reduced_row: &mut [i32],
    reduced_y: usize,
    vector_blocks: &mut u64,
    scalar_tail: &mut u64,
) {
    let top_start = reduced_y * 2 * source_width;
    let bottom_start = top_start + source_width;
    let top = &source[top_start..top_start + source_width];
    let bottom = &source[bottom_start..bottom_start + source_width];
    let mut output_x = 0usize;
    while reduced_row.len() - output_x >= LANES {
        let source_x = output_x * 2;
        // SAFETY: this strategy checks even source dimensions and requires at least one complete
        // block. The row pair contains 32 readable samples from both top and bottom pointers.
        let values = unsafe {
            crate::compute::pool_cpu::ops::reduce::helpers::i32_avx512f::reduce_quartet_block(
                top.as_ptr().add(source_x),
                bottom.as_ptr().add(source_x),
            )
        };
        reduced_row[output_x..output_x + LANES].copy_from_slice(&values);
        output_x += LANES;
        *vector_blocks += 1;
    }
    for (output, reduced) in reduced_row.iter_mut().enumerate().skip(output_x) {
        *reduced = super::super::helpers::streaming::reduce_i32_sample(top, bottom, output * 2);
    }
    *scalar_tail += (reduced_row.len() - output_x) as u64;
}
