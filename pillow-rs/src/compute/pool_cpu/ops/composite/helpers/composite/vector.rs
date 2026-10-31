//! Portable wide byte Composite kernel over exact native image modes.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use crate::compute::pool_cpu::ops::paste::helpers::vector::blend_l_masked_block16;

pub(super) fn execute(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
    mode: Mode,
    channels: u8,
) -> Result<DynamicImage, PilError> {
    if mask.dimensions() != source.dimensions() {
        return Err(PilError::InternalError(
            "Composite vector strategy received a mismatched mask".into(),
        ));
    }
    let mask_samples = super::mask::samples(mask, mask_alpha)?;
    let (source_width, source_height) = source.dimensions();
    let (destination_width, destination_height) = destination.dimensions();
    let source_dimensions = CheckedDims::new_allow_empty(source_width, source_height, channels)?;
    let destination_dimensions =
        CheckedDims::new_allow_empty(destination_width, destination_height, channels)?;
    let source_bytes = source.as_bytes();
    let destination_bytes = destination.as_bytes();
    let source_pixels = source_dimensions.total_bytes() / usize::from(channels);
    if source.mode() != mode
        || destination.mode() != mode
        || source_bytes.len() != source_dimensions.total_bytes()
        || destination_bytes.len() != destination_dimensions.total_bytes()
        || mask_samples.len() != source_pixels
    {
        return Err(PilError::InternalError(
            "Composite vector strategy received an invalid prepared layout".into(),
        ));
    }

    let channels = usize::from(channels);
    let overlap_width = source_width.min(destination_width) as usize;
    let overlap_height = source_height.min(destination_height) as usize;
    let row_bytes = overlap_width
        .checked_mul(channels)
        .ok_or_else(|| PilError::InternalError("Composite row size overflow".into()))?;
    let source_width = source_width as usize;
    let destination_width = destination_width as usize;
    let mut output = destination_bytes.to_vec();
    if row_bytes == 0 || overlap_height == 0 {
        crate::compute::record_pipeline_operation_path("native-copy");
        return crate::compute::pool_cpu::ops::composite::helpers::native_bytes::construct(
            destination_width as u32,
            destination_height,
            mode,
            output,
        );
    }

    let mut vector_blocks = 0u64;
    for row in 0..overlap_height {
        let source_row_start = row * source_width * channels;
        let destination_row_start = row * destination_width * channels;
        let mask_row_start = row * source_width;
        for start in (0..row_bytes).step_by(16) {
            let active = (row_bytes - start).min(16);
            let mut source_block = [0u8; 16];
            let mut destination_block = [0u8; 16];
            let mut mask_block = [0u8; 16];
            source_block[..active]
                .copy_from_slice(&source_bytes[source_row_start + start..source_row_start + start + active]);
            destination_block[..active].copy_from_slice(
                &output[destination_row_start + start..destination_row_start + start + active],
            );
            for (lane, mask_sample) in mask_block.iter_mut().enumerate().take(active) {
                *mask_sample = mask_samples.get(mask_row_start + (start + lane) / channels);
            }
            let blended =
                blend_l_masked_block16(source_block, destination_block, mask_block);
            output[destination_row_start + start..destination_row_start + start + active]
                .copy_from_slice(&blended[..active]);
            vector_blocks = vector_blocks.saturating_add(1);
        }
    }

    let scalar_tail = row_bytes % 16 * overlap_height;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::pool_cpu::ops::composite::helpers::native_bytes::construct(
        destination_width as u32,
        destination_height,
        mode,
        output,
    )
}
