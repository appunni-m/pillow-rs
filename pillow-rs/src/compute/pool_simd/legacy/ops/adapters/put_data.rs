//! SIMD PutData prefix replacement and native-layout admission.

use super::paste::{
    NativePasteLayout, native_paste_actual_layout, native_paste_layout, native_paste_shape_layout,
};
use super::{
    DynamicImage, PilError, PipelineOp, SimdImageShape, copy_native_bytes, pixel_mode_name,
    preserve_mode, simd_unsupported,
};
use crate::pipeline::PixelMode;

fn native_put_data_layout(
    img: &DynamicImage,
    data_mode: PixelMode,
    mode: Option<&str>,
) -> Option<NativePasteLayout> {
    let logical_mode = pixel_mode_name(data_mode);
    let layout =
        native_paste_layout(img, mode).or_else(|| native_paste_layout(img, Some(logical_mode)))?;
    (layout.channels == data_mode.channels()).then_some(layout)
}

fn native_put_data_shape_layout(
    shape: SimdImageShape,
    data_mode: PixelMode,
    mode: Option<&str>,
) -> Option<NativePasteLayout> {
    let logical_mode = pixel_mode_name(data_mode);
    let layout = native_paste_shape_layout(shape, mode)
        .or_else(|| native_paste_shape_layout(shape, Some(logical_mode)))?;
    (layout.channels == data_mode.channels()).then_some(layout)
}

pub(super) fn supports_for_image(
    img: &DynamicImage,
    data_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    native_put_data_layout(img, data_mode, mode)
        .is_some_and(|layout| native_paste_actual_layout(img, layout))
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    data_mode: PixelMode,
    mode: Option<&str>,
) -> bool {
    native_put_data_shape_layout(shape, data_mode, mode).is_some()
}

/// Replace the canonical `putdata` prefix through the image's native byte
/// layout. The binding/core boundary has already normalized scalar and tuple
/// values into `data`; the SIMD adapter only performs the defined byte copy,
/// preserving untouched pixels when Pillow receives a short sequence.
pub fn simd_put_data(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::PutData {
        data,
        mode: data_mode,
    } = op
    else {
        return Err(PilError::ValueError("expected PutData op".into()));
    };
    let layout =
        native_put_data_layout(img, *data_mode, mode).ok_or_else(|| simd_unsupported("PutData"))?;
    if !native_paste_actual_layout(img, layout) {
        return Err(simd_unsupported("PutData"));
    }
    let expected_len = img
        .width()
        .checked_mul(img.height())
        .and_then(|pixels| pixels.checked_mul(layout.channels as u32))
        .map(|bytes| bytes as usize)
        .ok_or_else(|| PilError::ValueError("SIMD PutData byte count overflow".into()))?;
    if img.as_bytes().len() != expected_len {
        return Err(simd_unsupported("PutData"));
    }

    let copy_len = data.len().min(expected_len);
    let (output, vector_blocks, scalar_tail) = if copy_len == expected_len {
        // A complete payload replaces the entire image. Copy it once into
        // the result instead of cloning source samples that are immediately
        // overwritten, then copying the payload a second time.
        (data[..copy_len].to_vec(), 0, 0)
    } else {
        let mut output = vec![0u8; expected_len];
        let (mut vector_blocks, mut scalar_tail) = copy_native_bytes(img.as_bytes(), &mut output)
            .ok_or_else(|| {
            PilError::InternalError("SIMD PutData buffer shape mismatch".into())
        })?;
        if copy_len != 0 {
            let (blocks, tail) = copy_native_bytes(&data[..copy_len], &mut output[..copy_len])
                .ok_or_else(|| {
                    PilError::InternalError("SIMD PutData prefix shape mismatch".into())
                })?;
            vector_blocks = vector_blocks.saturating_add(blocks);
            scalar_tail = scalar_tail.saturating_add(tail);
        }
        (output, vector_blocks, scalar_tail)
    };
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    let result = crate::image_utils::raw_bytes_to_image_allow_empty(
        img.width(),
        img.height(),
        output,
        layout.channels,
    )?;
    Ok(preserve_mode(img, result))
}
