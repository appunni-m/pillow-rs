//! Native byte point operations that share ImageEnhance's execution contract.

use super::super::{
    DynamicImage, PipelineOp, SimdImageShape, apply_native_rows, has_nonempty_byte_data,
    native_byte_layout, native_byte_transform_bytes, native_byte_transform_copy_bytes,
    shape_has_nonempty_byte_data, shape_native_byte_channels, simd_unsupported,
};
use crate::error::PilError;
use wide::u8x16;

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    matches!(
        op,
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. }
    ) && native_byte_layout(img, mode).is_some_and(|channels| has_nonempty_byte_data(img, channels))
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    op: &PipelineOp,
    mode: Option<&str>,
) -> bool {
    matches!(
        op,
        PipelineOp::Solarize { .. } | PipelineOp::Posterize { .. }
    ) && shape_native_byte_channels(shape, mode)
        .is_some_and(|channels| shape_has_nonempty_byte_data(shape, channels))
}

pub(super) fn supports_in_place(img: &DynamicImage, mode: Option<&str>) -> bool {
    native_byte_layout(img, mode).is_some_and(|channels| has_nonempty_byte_data(img, channels))
}

fn native_byte_transform<F>(
    img: &DynamicImage,
    mode: Option<&str>,
    transform: F,
) -> Option<DynamicImage>
where
    F: Fn(u8x16) -> u8x16,
{
    let channels = native_byte_layout(img, mode)?;
    if !has_nonempty_byte_data(img, channels) {
        return None;
    }
    let result = match img {
        DynamicImage::ImageLuma8(image) if matches!(mode, None | Some("L")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 1, &transform);
            Some(DynamicImage::ImageLuma8(result))
        }
        DynamicImage::ImageLumaA8(image) if matches!(mode, None | Some("LA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 2, &transform);
            Some(DynamicImage::ImageLumaA8(result))
        }
        DynamicImage::ImageRgb8(image) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 3, &transform);
            Some(DynamicImage::ImageRgb8(result))
        }
        DynamicImage::ImageRgba8(image) if matches!(mode, None | Some("RGBA")) => {
            let mut result = image.clone();
            native_byte_transform_bytes(&mut result, 4, &transform);
            Some(DynamicImage::ImageRgba8(result))
        }
        _ => None,
    }?;
    let vector_blocks = img.as_bytes().len().div_ceil(16) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_path("vector");
    Some(result)
}

/// Apply Solarize or Posterize to an owned intermediate without cloning it.
/// This is reached only after the family admission check succeeds.
fn native_byte_transform_in_place<F>(
    img: &mut DynamicImage,
    mode: Option<&str>,
    transform: F,
) -> bool
where
    F: Fn(u8x16) -> u8x16 + Send + Sync,
{
    let Some(channels) = native_byte_layout(img, mode) else {
        return false;
    };
    let width = img.width() as usize;
    let height = img.height() as usize;
    if width == 0 || height == 0 {
        return false;
    }
    let Some(bytes) = img.as_bytes_mut() else {
        return false;
    };
    let row_bytes = width.saturating_mul(channels);
    let vector_blocks = row_bytes.div_ceil(16).saturating_mul(height) as u64;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    apply_native_rows(bytes, width, height, channels, |row| {
        native_byte_transform_bytes(row, channels, &transform);
    });
    crate::compute::record_pipeline_operation_path("vector");
    true
}

pub(super) fn execute_in_place(
    img: &mut DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<bool, PilError> {
    match op {
        PipelineOp::Solarize { threshold } => {
            Ok(native_byte_transform_in_place(img, mode, |input| {
                input
                    .simd_ge(u8x16::splat(*threshold))
                    .select(u8x16::splat(u8::MAX) - input, input)
            }))
        }
        PipelineOp::Posterize { bits } => {
            let shift = 8u32
                .checked_sub(*bits as u32)
                .ok_or_else(|| PilError::ValueError("posterize bits must be at most 8".into()))?;
            Ok(native_byte_transform_in_place(img, mode, |input| {
                (input >> shift) << shift
            }))
        }
        _ => Ok(false),
    }
}

pub fn simd_solarize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Solarize { threshold } = op else {
        return Err(PilError::ValueError("expected Solarize op".into()));
    };
    if let Some(result) = native_byte_transform(img, mode, |input| {
        input
            .simd_ge(u8x16::splat(*threshold))
            .select(u8x16::splat(u8::MAX) - input, input)
    }) {
        return Ok(result);
    }
    Err(simd_unsupported("Solarize"))
}

pub fn simd_posterize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Posterize { bits } = op else {
        return Err(PilError::ValueError("expected Posterize op".into()));
    };

    // ImageOps.posterize validates the public mode before it queues this
    // operation: only L and RGB reach the pipeline, and both use the native
    // byte layouts above. Keep the explicit error for invalid internal calls.
    let shift = 8u32
        .checked_sub(*bits as u32)
        .ok_or_else(|| PilError::ValueError("posterize bits must be at most 8".into()))?;
    let mask = !((1u8 << shift) - 1);
    let channels = match (img, mode) {
        (DynamicImage::ImageLuma8(_), None | Some("L")) => 1,
        (DynamicImage::ImageRgb8(_), None | Some("RGB")) => 3,
        _ => 0,
    };
    if channels == 0 || !has_nonempty_byte_data(img, channels) {
        return Err(PilError::NotImplementedError(
            "SIMD posterize requires a validated native L or RGB byte image".into(),
        ));
    }
    let source = img.as_bytes();
    let mut bytes = vec![0u8; source.len()];
    native_byte_transform_copy_bytes(source, &mut bytes, &|input| input & u8x16::splat(mask));
    let vector_blocks = source.len().div_ceil(16) as u64;
    let result =
        crate::image_utils::raw_bytes_to_image(img.width(), img.height(), bytes, channels)?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(crate::image::preserve_mode(img, result))
}
