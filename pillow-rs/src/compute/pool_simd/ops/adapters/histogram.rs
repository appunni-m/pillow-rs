//! SIMD histogram adapters for autocontrast and equalization.

use super::{
    SimdImageShape, SimdLayout, has_valid_byte_data, native_lut_apply, native_lut_is_identity,
    shape_channels, shape_has_empty_native_bytes, shape_has_nonempty_byte_data, simd_unsupported,
};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;
use std::sync::Arc;

fn native_autocontrast_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB")) => Some(3),
        _ => None,
    }
}

fn shape_native_autocontrast_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    let channels = shape_channels(shape)?;
    let valid = match shape.layout {
        SimdLayout::Luma8 => matches!(mode, None | Some("L")),
        SimdLayout::Rgb8 => matches!(mode, None | Some("RGB")),
        SimdLayout::LumaA8 | SimdLayout::Rgba8 | SimdLayout::Luma16 | SimdLayout::Unsupported => {
            false
        }
    };
    valid.then_some(channels)
}

fn autocontrast_mask_supported(width: u32, height: u32, mask: Option<&Arc<Image>>) -> bool {
    let Some(mask) = mask else {
        return true;
    };
    matches!(mask.mode().ok().as_deref(), Some("1") | Some("L"))
        && mask.size().ok() == Some((width, height))
}

pub(super) fn supports_for_image(img: &DynamicImage, op: &PipelineOp, mode: Option<&str>) -> bool {
    match op {
        PipelineOp::Autocontrast { cutoff, mask } => {
            cutoff.is_finite()
                && native_autocontrast_layout(img, mode)
                    .is_some_and(|channels| has_valid_byte_data(img, channels))
                && autocontrast_mask_supported(img.width(), img.height(), mask.as_ref())
        }
        PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => {
            native_autocontrast_layout(img, mode)
                .is_some_and(|channels| has_valid_byte_data(img, channels))
                && match op {
                    PipelineOp::EqualizeMasked { mask } => {
                        autocontrast_mask_supported(img.width(), img.height(), Some(mask))
                    }
                    _ => true,
                }
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
        PipelineOp::Autocontrast { cutoff, mask } => {
            cutoff.is_finite()
                && shape_native_autocontrast_channels(shape, mode).is_some_and(|channels| {
                    shape_has_empty_native_bytes(shape, channels)
                        || shape_has_nonempty_byte_data(shape, channels)
                })
                && autocontrast_mask_supported(shape.width, shape.height, mask.as_ref())
        }
        PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. } => {
            shape_native_autocontrast_channels(shape, mode).is_some_and(|channels| {
                shape_has_empty_native_bytes(shape, channels)
                    || shape_has_nonempty_byte_data(shape, channels)
            }) && match op {
                PipelineOp::EqualizeMasked { mask } => {
                    autocontrast_mask_supported(shape.width, shape.height, Some(mask))
                }
                _ => true,
            }
        }
        _ => false,
    }
}

/// Build the histogram and percentile LUT with the shared scalar control
/// plane, then apply every native L/RGB byte through the vector LUT kernel.
/// Masks affect only LUT construction; they do not force the output pass back
/// through the CPU pixel implementation.
pub(crate) fn simd_autocontrast(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Autocontrast { cutoff, mask } = op else {
        return Err(PilError::ValueError("expected Autocontrast op".into()));
    };
    let Some(channels) = native_autocontrast_layout(img, mode) else {
        return Err(simd_unsupported("Autocontrast"));
    };
    if !cutoff.is_finite() || !autocontrast_mask_supported(img.width(), img.height(), mask.as_ref())
    {
        return Err(simd_unsupported("Autocontrast"));
    }
    if !has_valid_byte_data(img, channels) {
        return Err(simd_unsupported("Autocontrast"));
    }
    if img.width() == 0 || img.height() == 0 {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(img.clone());
    }

    let lut =
        crate::compute::pool_cpu::ops::imageops::autocontrast_lut(img, *cutoff, mask.as_ref())?;
    let mut result = img.clone();
    let Some(bytes) = result.as_bytes_mut() else {
        return Err(simd_unsupported("Autocontrast"));
    };
    let Some((vector_blocks, scalar_tail)) = native_lut_apply(
        bytes,
        img.width() as usize,
        img.height() as usize,
        channels,
        &lut,
    ) else {
        return Err(simd_unsupported("Autocontrast"));
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path("vector");
    Ok(result)
}

/// Apply Pillow's equalize LUT directly to native L/RGB storage.
/// Histogram construction is shared scalar control; every stored output byte
/// is then handled by the vector LUT kernel without an RGB widening copy.
pub(crate) fn simd_equalize(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    if !matches!(op, PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }) {
        return Err(PilError::ValueError("expected Equalize op".into()));
    }
    let Some(channels) = native_autocontrast_layout(img, mode) else {
        return Err(simd_unsupported("Equalize"));
    };
    if !has_valid_byte_data(img, channels) {
        return Err(simd_unsupported("Equalize"));
    }
    if img.width() == 0 || img.height() == 0 {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(img.clone());
    }
    let mask = match op {
        PipelineOp::EqualizeMasked { mask } => Some(mask.materialized_shared()?),
        _ => None,
    };
    let Some(lut) =
        crate::compute::pool_cpu::ops::imageops::equalize_lut(img, channels, mask.as_deref())
    else {
        return Err(simd_unsupported("Equalize"));
    };
    if native_lut_is_identity(&lut, channels) {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(img.clone());
    }
    let mut result = img.clone();
    let Some(bytes) = result.as_bytes_mut() else {
        return Err(simd_unsupported("Equalize"));
    };
    let source = img.as_bytes();
    let (vector_blocks, scalar_tail, path) = if channels == 1 && source.len() <= 4096 {
        // The portable byte-vector LUT expands each lookup into sixteen
        // swizzles and lane selects. For small frames, a direct L1 table read
        // avoids that setup and shuffle cost; larger images retain the vector
        // path where its work amortizes over enough pixels.
        for (output, &value) in bytes.iter_mut().zip(source) {
            *output = lut[usize::from(value)];
        }
        (0, source.len() as u64, "scalar-lut")
    } else {
        let Some((vector_blocks, scalar_tail)) = native_lut_apply(
            bytes,
            img.width() as usize,
            img.height() as usize,
            channels,
            &lut,
        ) else {
            return Err(simd_unsupported("Equalize"));
        };
        (vector_blocks, scalar_tail, "vector")
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path(path);
    Ok(result)
}
