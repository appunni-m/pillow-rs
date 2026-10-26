//! SIMD Crop and CropBorder family adapters with native-layout copy paths.

use super::{
    SimdImageShape, copy_native_bytes, has_valid_luma16_copy_bytes, native_copy_layout,
    native_luma16_image_from_bytes, shape_luma16_copy_layout, shape_native_copy_channels,
    simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GenericImageView};

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    let pixel_count = (img.width() as usize).saturating_mul(img.height() as usize);
    let native_copy = native_copy_layout(img, mode).is_some();
    let native_luma16_copy =
        has_valid_luma16_copy_bytes(img, mode) && img.width() != 0 && img.height() != 0;
    match op {
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            (native_copy || native_luma16_copy)
                && pixel_count != 0
                && *left < *right
                && *top < *bottom
                && *right <= img.width()
                && *bottom <= img.height()
        }
        PipelineOp::CropBorder { .. } => {
            // Keep invalid border geometry in the adapter so it can return
            // Pillow's ValueError instead of a SIMD capability error.
            native_copy
        }
        _ => false,
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    let pixel_count = (shape.width as usize).saturating_mul(shape.height as usize);
    let native_copy = shape_native_copy_channels(shape, mode).is_some();
    let native_luma16_copy =
        shape_luma16_copy_layout(shape, mode) && shape.width != 0 && shape.height != 0;
    match op {
        PipelineOp::Crop {
            left,
            top,
            right,
            bottom,
        } => {
            (native_copy || native_luma16_copy)
                && pixel_count != 0
                && *left < *right
                && *top < *bottom
                && *right <= shape.width
                && *bottom <= shape.height
        }
        PipelineOp::CropBorder { .. } => native_copy,
        _ => false,
    }
}

#[inline(always)]
fn copy_native_crop_rows(
    source: &[u8],
    output: &mut Vec<u8>,
    left: u32,
    top: u32,
    right: u32,
    image_width: u32,
    channels: usize,
    crop_height: u32,
    source_stride: usize,
    output_stride: usize,
    output_len: usize,
) -> Result<(), PilError> {
    if left == 0 && right == image_width {
        let source_start = (top as usize)
            .checked_mul(source_stride)
            .ok_or_else(|| PilError::ValueError("SIMD crop source offset overflow".into()))?;
        let source_end = source_start
            .checked_add(output_len)
            .ok_or_else(|| PilError::ValueError("SIMD crop source range overflow".into()))?;
        let source_span = source
            .get(source_start..source_end)
            .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?;
        output.extend_from_slice(source_span);
    } else {
        for y in 0..crop_height as usize {
            let source_start = (top as usize + y)
                .checked_mul(source_stride)
                .and_then(|offset| offset.checked_add(left as usize * channels))
                .ok_or_else(|| PilError::ValueError("SIMD crop source offset overflow".into()))?;
            let source_end = source_start
                .checked_add(output_stride)
                .ok_or_else(|| PilError::ValueError("SIMD crop source range overflow".into()))?;
            let source_row = source
                .get(source_start..source_end)
                .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?;
            output.extend_from_slice(source_row);
        }
    }
    Ok(())
}

/// Copy a validated crop directly in the source's native byte layout.
///
/// This is intentionally a `native-copy` kernel rather than arithmetic SIMD:
/// the useful optimization is preserving the interleaved L/LA/RGB/RGBA
/// buffer and copying complete row spans without an RGBA expansion.
fn native_crop_bytes(
    img: &DynamicImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    mode: Option<&str>,
    allow_empty: bool,
) -> Result<Option<DynamicImage>, PilError> {
    let Some(channels) = native_copy_layout(img, mode) else {
        return Ok(None);
    };
    let (image_width, image_height) = img.dimensions();
    if left > right || top > bottom || right > image_width || bottom > image_height {
        return Ok(None);
    }
    let width = right - left;
    let height = bottom - top;
    if width == 0 || height == 0 {
        if !allow_empty {
            return Ok(None);
        }
        crate::compute::record_pipeline_operation_path("native-copy");
        return crate::image_utils::raw_bytes_to_image_allow_empty(
            width,
            height,
            Vec::new(),
            channels,
        )
        .map(Some);
    }
    let source_stride = (image_width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD crop source stride overflow".into()))?;
    let output_stride = (width as usize)
        .checked_mul(channels)
        .ok_or_else(|| PilError::ValueError("SIMD crop output stride overflow".into()))?;
    let output_len = output_stride
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD crop output length overflow".into()))?;
    // Cropping is a full overwrite. Allocate capacity once instead of
    // zero-filling a buffer that the row copies immediately replace.
    let mut output = Vec::with_capacity(output_len);
    let source = img.as_bytes();
    copy_native_crop_rows(
        source,
        &mut output,
        left,
        top,
        right,
        image_width,
        channels,
        height,
        source_stride,
        output_stride,
        output_len,
    )?;
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::image_utils::raw_bytes_to_image(width, height, output, channels).map(Some)
}

/// Crop a typed `I;16*` image by copying complete two-byte sample groups.
/// This is a native-copy path: the vector kernel moves opaque samples and the
/// final reconstruction restores the typed `ImageLuma16` representation.
fn native_crop_luma16(
    img: &DynamicImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    mode: Option<&str>,
) -> Result<Option<DynamicImage>, PilError> {
    if !has_valid_luma16_copy_bytes(img, mode) {
        return Ok(None);
    }
    let (image_width, image_height) = img.dimensions();
    if left >= right || top >= bottom || right > image_width || bottom > image_height {
        return Ok(None);
    }
    let width = right - left;
    let height = bottom - top;
    let source_stride = (image_width as usize)
        .checked_mul(2)
        .ok_or_else(|| PilError::ValueError("SIMD crop source stride overflow".into()))?;
    let output_stride = (width as usize)
        .checked_mul(2)
        .ok_or_else(|| PilError::ValueError("SIMD crop output stride overflow".into()))?;
    let output_len = output_stride
        .checked_mul(height as usize)
        .ok_or_else(|| PilError::ValueError("SIMD crop output length overflow".into()))?;
    let source = img.as_bytes();
    let mut output = vec![0u8; output_len];
    let mut vector_blocks = 0u64;
    let mut scalar_tail = 0u64;
    for y in 0..height as usize {
        let source_start = (top as usize + y)
            .checked_mul(source_stride)
            .and_then(|offset| offset.checked_add(left as usize * 2))
            .ok_or_else(|| PilError::ValueError("SIMD crop source offset overflow".into()))?;
        let output_start = y * output_stride;
        let (blocks, tail) = copy_native_bytes(
            source
                .get(source_start..source_start + output_stride)
                .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?,
            output
                .get_mut(output_start..output_start + output_stride)
                .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?,
        )
        .ok_or_else(|| PilError::InternalError("SIMD crop buffer shape mismatch".into()))?;
        vector_blocks = vector_blocks.saturating_add(blocks);
        scalar_tail = scalar_tail.saturating_add(tail);
    }
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    Ok(native_luma16_image_from_bytes(width, height, output))
}

pub(crate) fn simd_crop_border(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::CropBorder { border } = op else {
        return Err(PilError::ValueError("expected CropBorder op".into()));
    };
    // CropBorder is a contiguous byte movement, not an arithmetic SIMD
    // workload. On ordinary native layouts, avoid widening every source
    // pixel to the packed RGBA representation only to copy it back out.
    if native_copy_layout(img, mode).is_some() {
        let (w, h) = img.dimensions();
        // Avoid the packed adapter's representation conversion while
        // preserving ImageOps.crop's public error text.  The checked
        // half-size form is equivalent to Pillow's `2 * border > size`
        // rule without allowing a u32 multiplication to wrap.
        if *border > w / 2 {
            return Err(PilError::ValueError(
                "Coordinate 'right' is less than 'left'".into(),
            ));
        }
        if *border > h / 2 {
            return Err(PilError::ValueError(
                "Coordinate 'lower' is less than 'upper'".into(),
            ));
        }
        if let Some(result) =
            native_crop_bytes(img, *border, *border, w - *border, h - *border, mode, true)?
        {
            return Ok(result);
        }
    }
    Err(simd_unsupported("CropBorder"))
}

pub(crate) fn simd_crop(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Crop {
        left,
        top,
        right,
        bottom,
    } = op
    else {
        return Err(PilError::ValueError("expected Crop op".into()));
    };
    // Crop is a byte movement, not an arithmetic SIMD workload. The native
    // implementation copies rows directly for ordinary byte layouts; typed
    // and empty-output layouts are rejected by contextual preflight instead
    // of entering a packed scalar implementation.
    if let Some(result) = native_crop_bytes(img, *left, *top, *right, *bottom, mode, false)? {
        return Ok(result);
    }
    if let Some(result) = native_crop_luma16(img, *left, *top, *right, *bottom, mode)? {
        return Ok(result);
    }
    Err(simd_unsupported("Crop"))
}
