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
#[derive(Clone, Copy)]
struct NativeExpandLayout {
    border: usize,
    channels: usize,
    source_height: usize,
    output_width: u32,
    output_height: u32,
    output_width_usize: usize,
    output_height_usize: usize,
    source_stride: usize,
    output_stride: usize,
    output_len: usize,
}

fn native_expand_identity(
    source: &[u8],
    layout: NativeExpandLayout,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let mut output = vec![0u8; layout.output_len];
    let (vector_blocks, scalar_tail) = copy_native_bytes(source, &mut output)
        .ok_or_else(|| PilError::InternalError("SIMD expand copy shape mismatch".into()))?;
    let result = crate::image_utils::raw_bytes_to_image(
        layout.output_width,
        layout.output_height,
        output,
        layout.channels,
    )?;
    Ok((result, vector_blocks, scalar_tail))
}

fn native_expand_luma(
    source: &[u8],
    fill: (u8, u8, u8, u8),
    layout: NativeExpandLayout,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let mut fill_row = vec![0u8; layout.output_stride];
    let (row_blocks, row_tail) = native_fill_row(&mut fill_row, fill, layout.channels)
        .ok_or_else(|| PilError::InternalError("SIMD expand fill row mismatch".into()))?;
    let mut fill_sides = vec![0u8; layout.border];
    let (side_blocks, side_tail) = native_fill_row(&mut fill_sides, fill, layout.channels)
        .ok_or_else(|| PilError::InternalError("SIMD expand fill sides mismatch".into()))?;
    let source_end_y = layout
        .border
        .checked_add(layout.source_height)
        .ok_or_else(|| PilError::ValueError("SIMD expand source rows overflow".into()))?;
    let mut output = Vec::with_capacity(layout.output_len);
    for y in 0..layout.output_height_usize {
        if y < layout.border || y >= source_end_y {
            output.extend_from_slice(&fill_row);
        } else {
            output.extend_from_slice(&fill_sides);
            let source_y = y - layout.border;
            let source_start = source_y * layout.source_stride;
            output.extend_from_slice(&source[source_start..source_start + layout.source_stride]);
            output.extend_from_slice(&fill_sides);
        }
    }
    if output.len() != layout.output_len {
        return Err(PilError::InternalError(
            "SIMD expand L output shape mismatch".into(),
        ));
    }
    let result = crate::image_utils::raw_bytes_to_image(
        layout.output_width,
        layout.output_height,
        output,
        layout.channels,
    )?;
    Ok((
        result,
        row_blocks.saturating_add(side_blocks),
        row_tail.saturating_add(side_tail),
    ))
}

fn native_expand_interleaved(
    source: &[u8],
    fill: (u8, u8, u8, u8),
    layout: NativeExpandLayout,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let fill_sample = [fill.0, fill.1, fill.2, fill.3];
    let fill_pixel = &fill_sample[..layout.channels];
    let fill_row = fill_pixel.repeat(layout.output_width_usize);
    let fill_sides = fill_pixel.repeat(layout.border);
    let border_bytes = layout
        .border
        .checked_mul(layout.channels)
        .ok_or_else(|| PilError::ValueError("SIMD expand border stride overflow".into()))?;
    let source_end_y = layout
        .border
        .checked_add(layout.source_height)
        .ok_or_else(|| PilError::ValueError("SIMD expand source rows overflow".into()))?;
    let mut output = Vec::with_capacity(layout.output_len);
    for y in 0..layout.output_height_usize {
        if y < layout.border || y >= source_end_y {
            output.extend_from_slice(&fill_row);
            continue;
        }
        output.extend_from_slice(&fill_sides);
        let source_y = y - layout.border;
        let source_start = source_y * layout.source_stride;
        output.extend_from_slice(&source[source_start..source_start + layout.source_stride]);
        output.extend_from_slice(&fill_sides);
    }
    if output.len() != layout.output_len
        || fill_row.len() != layout.output_stride
        || fill_sides.len() != border_bytes
    {
        return Err(PilError::InternalError(
            "SIMD expand native row shape mismatch".into(),
        ));
    }
    let result = crate::image_utils::raw_bytes_to_image(
        layout.output_width,
        layout.output_height,
        output,
        layout.channels,
    )?;
    Ok((result, 0, 0))
}

fn native_expand_other_bytes(
    source: &[u8],
    fill: (u8, u8, u8, u8),
    layout: NativeExpandLayout,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let mut output = vec![0u8; layout.output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    let border_bytes = layout
        .border
        .checked_mul(layout.channels)
        .ok_or_else(|| PilError::ValueError("SIMD expand border stride overflow".into()))?;
    let source_end_y = layout
        .border
        .checked_add(layout.source_height)
        .ok_or_else(|| PilError::ValueError("SIMD expand source rows overflow".into()))?;

    // Fill only the actual border. Filling the full destination and then
    // copying the source over its center writes every source byte twice.
    for y in 0..layout.output_height_usize {
        let output_start = y * layout.output_stride;
        let row = &mut output[output_start..output_start + layout.output_stride];
        if y < layout.border || y >= source_end_y {
            let (blocks, tail) = native_fill_row(row, fill, layout.channels)
                .ok_or_else(|| PilError::InternalError("SIMD expand fill shape mismatch".into()))?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
            continue;
        }

        let (blocks, tail) = native_fill_row(&mut row[..border_bytes], fill, layout.channels)
            .ok_or_else(|| PilError::InternalError("SIMD expand fill shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);

        let source_y = y - layout.border;
        let source_start = source_y * layout.source_stride;
        let source_end = source_start + layout.source_stride;
        let copy_start = border_bytes;
        let copy_end = copy_start + layout.source_stride;
        let (blocks, tail) = copy_native_bytes(
            &source[source_start..source_end],
            &mut row[copy_start..copy_end],
        )
        .ok_or_else(|| PilError::InternalError("SIMD expand copy shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);

        let (blocks, tail) = native_fill_row(&mut row[copy_end..], fill, layout.channels)
            .ok_or_else(|| PilError::InternalError("SIMD expand fill shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    let result = crate::image_utils::raw_bytes_to_image(
        layout.output_width,
        layout.output_height,
        output,
        layout.channels,
    )?;
    Ok((result, vector_blocks, scalar_tail))
}

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
    let layout = NativeExpandLayout {
        border: border as usize,
        channels,
        source_height,
        output_width,
        output_height,
        output_width_usize,
        output_height_usize,
        source_stride,
        output_stride,
        output_len,
    };

    // ImageOps.expand(..., border=0) is an identity copy. Keep the native
    // copy contract and telemetry while skipping the redundant fill pass.
    let result = if border == 0 {
        native_expand_identity(source, layout)?
    } else if channels == 1 {
        // L/P indices use vector-filled rows and remain unconverted.
        native_expand_luma(source, fill, layout)?
    } else if channels == 3 || channels == 4 {
        // Interleaved samples use precomputed fill spans and append each row
        // once. RGBX X and CMYK K remain ordinary stored bytes.
        native_expand_interleaved(source, fill, layout)?
    } else {
        native_expand_other_bytes(source, fill, layout)?
    };
    Ok(Some(result))
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
