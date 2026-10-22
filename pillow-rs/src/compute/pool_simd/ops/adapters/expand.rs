//! SIMD Expand family adapter and native-layout output construction.

use super::{
    SimdImageShape, copy_native_bytes, native_expand_contract_for_image,
    native_expand_contract_for_shape, native_fill_row, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Expand { border, .. } => {
            native_expand_contract_for_image(img, *border, mode).is_some()
        }
        _ => false,
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    match op {
        PipelineOp::Expand { border, .. } => {
            native_expand_contract_for_shape(shape, *border, mode).is_some()
        }
        _ => false,
    }
}

/// Expand an image in its native byte layout using vectorized fill and row
/// copies. The output border is intentionally handled as two data-plane
/// kernels: a repeated `u8x16` fill pattern, followed by direct source-row
/// copies. This keeps P/PA indices raw and keeps CMYK's K sample in the fourth
/// byte; no intermediate RGBA image is created.
fn native_expand_bytes(
    img: &DynamicImage,
    border: u32,
    fill: (u8, u8, u8, u8),
    mode: Option<&str>,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    let Some((channels, output_width, output_height)) =
        native_expand_contract_for_image(img, border, mode)
    else {
        return Ok(None);
    };
    let source_width = img.width() as usize;
    let source_height = img.height() as usize;
    let output_width_usize = output_width as usize;
    let output_height_usize = output_height as usize;
    let source_stride = source_width
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD expand source stride overflow".into()))?;
    let output_stride = output_width_usize
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD expand output stride overflow".into()))?;
    let output_len = output_stride
        .checked_mul(output_height_usize)
        .ok_or_else(|| PilError::ValueError("SIMD expand output length overflow".into()))?;
    let source = img.as_bytes();
    if source.len() != source_stride * source_height {
        return Ok(None);
    }

    // ImageOps.expand(..., border=0) is an identity copy.  The general
    // border path would first fill every output row and then overwrite that
    // entire frame with the source, doubling the data-plane work for the
    // common no-border form.  Keep the native copy contract and telemetry,
    // but skip the redundant fill pass.
    if border == 0 {
        let mut output = vec![0u8; output_len];
        let (vector_blocks, scalar_tail) = copy_native_bytes(source, &mut output)
            .ok_or_else(|| PilError::InternalError("SIMD expand copy shape mismatch".into()))?;
        let result =
            crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
        return Ok(Some((result, vector_blocks, scalar_tail)));
    }

    // Single-byte L/P expansion used to zero-initialize the complete output,
    // then overwrite every byte with either the fill value or a copied sample.
    // Build the frame by appending precomputed vector-filled rows instead, so
    // each output byte is written once and palette indices remain untouched.
    if channels == 1 {
        let border_bytes = border as usize;
        let mut fill_row = vec![0u8; output_stride];
        let (row_blocks, row_tail) = native_fill_row(&mut fill_row, fill, channels)
            .ok_or_else(|| PilError::InternalError("SIMD expand fill row mismatch".into()))?;
        let mut fill_sides = vec![0u8; border_bytes];
        let (side_blocks, side_tail) = native_fill_row(&mut fill_sides, fill, channels)
            .ok_or_else(|| PilError::InternalError("SIMD expand fill sides mismatch".into()))?;
        let source_end_y = (border as usize)
            .checked_add(source_height)
            .ok_or_else(|| PilError::ValueError("SIMD expand source rows overflow".into()))?;
        let mut output = Vec::with_capacity(output_len);
        for y in 0..output_height_usize {
            if y < border as usize || y >= source_end_y {
                output.extend_from_slice(&fill_row);
            } else {
                output.extend_from_slice(&fill_sides);
                let source_y = y - border as usize;
                let source_start = source_y * source_stride;
                output.extend_from_slice(&source[source_start..source_start + source_stride]);
                output.extend_from_slice(&fill_sides);
            }
        }
        if output.len() != output_len {
            return Err(PilError::InternalError(
                "SIMD expand L output shape mismatch".into(),
            ));
        }
        let result =
            crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
        return Ok(Some((
            result,
            row_blocks.saturating_add(side_blocks),
            row_tail.saturating_add(side_tail),
        )));
    }

    // Three- and four-byte samples need only row construction and byte copies.
    // Building a u8x16 for every fill block repeats the fill pattern work,
    // while zero-initializing the full destination before overwriting its
    // source interior adds another complete frame write. Precompute native
    // fill spans and append each output row once. Four-byte formats retain
    // every source byte, including RGBX's non-alpha X and CMYK's K sample.
    if channels == 3 || channels == 4 {
        let fill_sample = [fill.0, fill.1, fill.2, fill.3];
        let fill_pixel = &fill_sample[..channels];
        let fill_row = fill_pixel.repeat(output_width_usize);
        let fill_sides = fill_pixel.repeat(border as usize);
        let border_bytes = (border as usize)
            .checked_mul(channels)
            .ok_or_else(|| PilError::ValueError("SIMD expand border stride overflow".into()))?;
        let source_end_y = (border as usize)
            .checked_add(source_height)
            .ok_or_else(|| PilError::ValueError("SIMD expand source rows overflow".into()))?;
        let mut output = Vec::with_capacity(output_len);
        for y in 0..output_height_usize {
            if y < border as usize || y >= source_end_y {
                output.extend_from_slice(&fill_row);
                continue;
            }
            output.extend_from_slice(&fill_sides);
            let source_y = y - border as usize;
            let source_start = source_y * source_stride;
            output.extend_from_slice(&source[source_start..source_start + source_stride]);
            output.extend_from_slice(&fill_sides);
        }
        if output.len() != output_len
            || fill_row.len() != output_stride
            || fill_sides.len() != border_bytes
        {
            return Err(PilError::InternalError(
                "SIMD expand native row shape mismatch".into(),
            ));
        }
        let result =
            crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
        return Ok(Some((result, 0, 0)));
    }

    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let border_usize = border as usize;
    let border_bytes = border_usize
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD expand border stride overflow".into()))?;
    let source_end_y = border_usize
        .checked_add(source_height)
        .ok_or_else(|| PilError::ValueError("SIMD expand source rows overflow".into()))?;

    // Fill only the actual border. Filling the full destination and then
    // copying the source over its center writes every source byte twice.
    for y in 0..output_height_usize {
        let output_start = y * output_stride;
        let row = &mut output[output_start..output_start + output_stride];
        if y < border_usize || y >= source_end_y {
            let (blocks, tail) = native_fill_row(row, fill, channels)
                .ok_or_else(|| PilError::InternalError("SIMD expand fill shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
            continue;
        }

        let (blocks, tail) = native_fill_row(&mut row[..border_bytes], fill, channels)
            .ok_or_else(|| PilError::InternalError("SIMD expand fill shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);

        let source_y = y - border_usize;
        let source_start = source_y * source_stride;
        let source_end = source_start + source_stride;
        let copy_start = border_bytes;
        let copy_end = copy_start + source_stride;
        let (blocks, tail) = copy_native_bytes(
            &source[source_start..source_end],
            &mut row[copy_start..copy_end],
        )
        .ok_or_else(|| PilError::InternalError("SIMD expand copy shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);

        let (blocks, tail) = native_fill_row(&mut row[copy_end..], fill, channels)
            .ok_or_else(|| PilError::InternalError("SIMD expand fill shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result =
        crate::image_utils::raw_bytes_to_image(output_width, output_height, output, channels)?;
    Ok(Some((result, vector_blocks, scalar_tail)))
}

pub(crate) fn simd_expand(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Expand { border, fill } = op else {
        return Err(PilError::ValueError("expected Expand op".into()));
    };
    let Some((result, vector_blocks, scalar_tail)) =
        native_expand_bytes(img, *border, *fill, mode)?
    else {
        return Err(simd_unsupported("Expand"));
    };
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(result)
}

#[cfg(test)]
mod tests;
