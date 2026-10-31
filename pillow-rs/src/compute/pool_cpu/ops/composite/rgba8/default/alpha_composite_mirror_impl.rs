//! Serial RGBA AlphaComposite followed by horizontal mirror in one pass.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;

/// Composites and mirrors native RGBA pixels without an intermediate image.
pub(crate) fn execute(
    source: &[u8],
    destination: &[u8],
    width: u32,
    height: u32,
) -> Result<Vec<u8>, PilError> {
    let dimensions = CheckedDims::new(width, height, 4)?;
    if source.len() != dimensions.total_bytes() || destination.len() != dimensions.total_bytes() {
        return Err(PilError::InternalError(
            "RGBA alpha composite mirror storage does not match its dimensions".into(),
        ));
    }

    let mut output = dimensions.alloc_buffer();
    if let (Some(source_pixel), Some(destination_pixel)) = (
        uniform_rgba_pixel(source),
        uniform_rgba_pixel(destination),
    ) {
        let mut result_pixel = destination_pixel;
        crate::compute::pool_cpu::ops::composite::helpers::alpha_composite::alpha_composite_pixel::<4>(
            &source_pixel,
            &mut result_pixel,
        );
        for pixel in output.chunks_exact_mut(4) {
            pixel.copy_from_slice(&result_pixel);
        }
        return Ok(output);
    }

    let width = width as usize;
    let row_stride = dimensions.row_stride();
    for (row_index, output_row) in output.chunks_exact_mut(row_stride).enumerate() {
        let row_start = row_index * row_stride;
        for (output_x, output_pixel) in output_row.chunks_exact_mut(4).enumerate() {
            let source_offset = row_start + (width - 1 - output_x) * 4;
            output_pixel.copy_from_slice(&destination[source_offset..source_offset + 4]);
            crate::compute::pool_cpu::ops::composite::helpers::alpha_composite::alpha_composite_pixel::<4>(
                &source[source_offset..source_offset + 4],
                output_pixel,
            );
        }
    }
    Ok(output)
}

fn uniform_rgba_pixel(bytes: &[u8]) -> Option<[u8; 4]> {
    let first: [u8; 4] = bytes.get(..4)?.try_into().ok()?;
    bytes
        .chunks_exact(4)
        .all(|pixel| pixel == first.as_slice())
        .then_some(first)
}
