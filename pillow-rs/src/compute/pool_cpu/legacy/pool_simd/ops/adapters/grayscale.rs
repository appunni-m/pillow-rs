//! Native ImageOps grayscale adapter.
//!
//! This module validates supported layouts and shapes, chooses the mode-specific
//! kernel, and records the resulting executor path. Pixel loops live in the
//! `kernels` submodule so adapter control flow stays separate from SIMD work.

use super::{SimdImageShape, SimdLayout, simd_unsupported};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

mod kernels;
pub(super) use self::kernels::{
    native_cmyk_grayscale_bytes, native_grayscale_bytes, native_ycbcr_grayscale_bytes,
    native_ycbcr_grayscale_path,
};

/// Return the native source layout accepted by ImageOps.grayscale.
///
/// Grayscale produces a new `L` image. RGBa has a dedicated integer
/// unpremultiplication pre-stage; ordinary RGBA ignores alpha, and RGBX's
/// fourth byte is padding. CMYK is admitted only with its dedicated
/// C/M/Y/K-to-luma kernel; treating its samples as RGBA would be incorrect.
/// Palette, other color-space, and typed modes remain rejected until their
/// mode-specific conversions can stay native.
fn native_grayscale_layout(img: &DynamicImage, mode: Option<&str>) -> Option<usize> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some(1),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some(2),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "YCbCr")) => Some(3),
        DynamicImage::ImageRgba8(_)
            if matches!(mode, None | Some("RGBA" | "RGBX" | "RGBa" | "CMYK")) =>
        {
            Some(4)
        }
        _ => None,
    }
}

fn native_grayscale_supported_for_image(img: &DynamicImage, mode: Option<&str>) -> bool {
    let Some(channels) = native_grayscale_layout(img, mode) else {
        return false;
    };
    let Some(pixels) = (img.width() as usize).checked_mul(img.height() as usize) else {
        return false;
    };
    pixels
        .checked_mul(channels)
        .is_some_and(|bytes| img.as_bytes().len() == bytes)
}

pub(super) fn supports_image(img: &DynamicImage, mode: Option<&str>) -> bool {
    native_grayscale_supported_for_image(img, mode)
}

pub(super) fn supports_shape(shape: SimdImageShape, mode: Option<&str>) -> bool {
    shape_native_grayscale_supported(shape, mode)
}

fn shape_native_grayscale_channels(shape: SimdImageShape, mode: Option<&str>) -> Option<usize> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some(1),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some(2),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "YCbCr")) => Some(3),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA" | "RGBX" | "RGBa" | "CMYK")) => {
            Some(4)
        }
        _ => None,
    }
}

fn shape_native_grayscale_supported(shape: SimdImageShape, mode: Option<&str>) -> bool {
    let Some(channels) = shape_native_grayscale_channels(shape, mode) else {
        return false;
    };
    shape
        .width
        .checked_mul(shape.height)
        .and_then(|pixels| pixels.checked_mul(channels as u32))
        .is_some()
}

pub(crate) fn simd_grayscale_with_evidence(
    img: &DynamicImage,
    _op: &PipelineOp,
    mode: Option<&str>,
) -> Result<(DynamicImage, u64, u64, &'static str), PilError> {
    let Some(channels) = native_grayscale_layout(img, mode) else {
        return Err(simd_unsupported("Grayscale"));
    };
    let (output, vector_blocks, scalar_tail) = if mode == Some("YCbCr") {
        let Some(result) = native_ycbcr_grayscale_bytes(img) else {
            return Err(simd_unsupported("Grayscale"));
        };
        result
    } else if mode == Some("CMYK") {
        let Some(result) = native_cmyk_grayscale_bytes(img) else {
            return Err(simd_unsupported("Grayscale"));
        };
        result
    } else {
        let unpremultiplied =
            matches!(mode, Some("RGBa")).then(|| crate::ops::pil_resize::unpremultiply_alpha(img));
        let source = unpremultiplied.as_ref().unwrap_or(img);
        let Some(result) = native_grayscale_bytes(source, channels) else {
            return Err(simd_unsupported("Grayscale"));
        };
        result
    };
    let path = if vector_blocks == 0 {
        "scalar-control"
    } else if mode == Some("YCbCr") {
        native_ycbcr_grayscale_path()
    } else if channels == 1 {
        "native-copy"
    } else {
        "vector"
    };
    if !output.is_empty() {
        // The specialized Vec-backed kernels above allocate the final L
        // buffer directly, outside CheckedDims::alloc_buffer.
        crate::compute::record_pipeline_allocation(output.len());
    }
    let result =
        crate::image_utils::raw_bytes_to_image_allow_empty(img.width(), img.height(), output, 1)?;
    Ok((result, vector_blocks, scalar_tail, path))
}

pub(crate) fn simd_grayscale(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let (result, vector_blocks, scalar_tail, path) = simd_grayscale_with_evidence(img, op, mode)?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail);
    crate::compute::record_pipeline_operation_path(path);
    Ok(result)
}

#[cfg(test)]
mod tests;
