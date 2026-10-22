//! SIMD Transform adapter admission and method selection.

mod kernels;

#[cfg(test)]
mod tests;

use self::kernels::{
    simd_affine_bilinear_transform_bytes, simd_affine_luma16_transform_bytes,
    simd_affine_nearest_transform_bytes, simd_mesh_transform_bytes,
    simd_projective_bilinear_transform_bytes, simd_projective_nearest_transform_bytes,
};
use super::{
    SimdImageShape, SimdLayout, native_copy_layout, shape_native_copy_channels, simd_unsupported,
};
use crate::error::PilError;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::DynamicImage;

fn native_affine_nearest_transform_channels(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<usize> {
    // Affine nearest sampling copies complete native samples.  Indexed and
    // color-space modes are therefore safe here even though they are not
    // admitted to arithmetic kernels: P copies indices, CMYK copies K as the
    // fourth sample, and I/F copy their four raw little-endian bytes.
    native_copy_layout(img, mode)
}

fn native_affine_nearest_transform_channels_for_shape(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<usize> {
    shape_native_copy_channels(shape, mode)
}

fn native_affine_bilinear_layout(
    img: &DynamicImage,
    mode: Option<&str>,
) -> Option<(usize, Option<usize>)> {
    match img {
        DynamicImage::ImageLuma8(_) if matches!(mode, None | Some("L")) => Some((1, None)),
        DynamicImage::ImageLumaA8(_) if matches!(mode, None | Some("LA")) => Some((2, Some(1))),
        DynamicImage::ImageRgb8(_) if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => {
            Some((3, None))
        }
        DynamicImage::ImageRgba8(_) if matches!(mode, None | Some("RGBA")) => Some((4, Some(3))),
        DynamicImage::ImageRgba8(_) if mode == Some("CMYK") => Some((4, None)),
        _ => None,
    }
}

fn shape_native_affine_bilinear_layout(
    shape: SimdImageShape,
    mode: Option<&str>,
) -> Option<(usize, Option<usize>)> {
    match shape.layout {
        SimdLayout::Luma8 if matches!(mode, None | Some("L")) => Some((1, None)),
        SimdLayout::LumaA8 if matches!(mode, None | Some("LA")) => Some((2, Some(1))),
        SimdLayout::Rgb8 if matches!(mode, None | Some("RGB" | "HSV" | "YCbCr")) => Some((3, None)),
        SimdLayout::Rgba8 if matches!(mode, None | Some("RGBA")) => Some((4, Some(3))),
        SimdLayout::Rgba8 if mode == Some("CMYK") => Some((4, None)),
        _ => None,
    }
}

fn native_affine_nearest_transform_supported_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let channels = match filter {
        ResampleFilter::Nearest => native_affine_nearest_transform_channels(img, mode),
        ResampleFilter::Bilinear => {
            native_affine_bilinear_layout(img, mode).map(|(channels, _)| channels)
        }
        _ => None,
    };
    let Some(channels) = channels else {
        return false;
    };
    if !matches!(method, TransformMethod::Affine)
        || width == 0
        || height == 0
        || data.len() != 6
        || data.iter().any(|value| !value.is_finite())
    {
        return false;
    }
    let Some(expected_bytes) = (img.width() as usize)
        .checked_mul(img.height() as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
    else {
        return false;
    };
    img.as_bytes().len() == expected_bytes
}

fn native_affine_nearest_transform_supported_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    let channels = match filter {
        ResampleFilter::Nearest => native_affine_nearest_transform_channels_for_shape(shape, mode),
        ResampleFilter::Bilinear => {
            shape_native_affine_bilinear_layout(shape, mode).map(|(channels, _)| channels)
        }
        _ => None,
    };
    channels.is_some()
        && matches!(method, TransformMethod::Affine)
        && width != 0
        && height != 0
        && data.len() == 6
        && data.iter().all(|value| value.is_finite())
}

fn native_affine_luma16_transform_supported_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    matches!(img, DynamicImage::ImageLuma16(_))
        && matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && matches!(filter, ResampleFilter::Nearest | ResampleFilter::Bilinear)
        && matches!(method, TransformMethod::Affine)
        && width != 0
        && height != 0
        && img.width() != 0
        && img.height() != 0
        && data.len() == 6
        && data.iter().all(|value| value.is_finite())
}

fn native_affine_luma16_transform_supported_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    shape.layout == SimdLayout::Luma16
        && matches!(mode, Some("I;16" | "I;16L" | "I;16B" | "I;16N"))
        && matches!(filter, ResampleFilter::Nearest | ResampleFilter::Bilinear)
        && matches!(method, TransformMethod::Affine)
        && width != 0
        && height != 0
        && shape.width != 0
        && shape.height != 0
        && data.len() == 6
        && data.iter().all(|value| value.is_finite())
}

fn native_projective_nearest_transform_supported_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    // The SIMD bilinear kernel does not yet reproduce Geometry.c's
    // center-minus-half, FLOOR, edge-clipped sampling contract. Let the
    // corrected CPU implementation own filtered projective transforms.
    matches!(method, TransformMethod::Perspective | TransformMethod::Quad)
        && matches!(filter, ResampleFilter::Nearest)
        && width != 0
        && height != 0
        && data.len() == 8
        && native_affine_nearest_transform_channels(img, mode).is_some_and(|channels| {
            (img.width() as usize)
                .checked_mul(img.height() as usize)
                .and_then(|pixels| pixels.checked_mul(channels))
                .is_some_and(|bytes| img.as_bytes().len() == bytes)
        })
}

fn native_projective_nearest_transform_supported_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    matches!(method, TransformMethod::Perspective | TransformMethod::Quad)
        && matches!(filter, ResampleFilter::Nearest)
        && width != 0
        && height != 0
        && data.len() == 8
        && native_affine_nearest_transform_channels_for_shape(shape, mode).is_some()
}

fn native_mesh_transform_supported_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    matches!(filter, ResampleFilter::Nearest)
        && width != 0
        && height != 0
        && img.width() != 0
        && img.height() != 0
        && !data.is_empty()
        && data.len() % 12 == 0
        && data.iter().all(|value| value.is_finite())
        && native_affine_nearest_transform_channels(img, mode).is_some_and(|channels| {
            (img.width() as usize)
                .checked_mul(img.height() as usize)
                .and_then(|pixels| pixels.checked_mul(channels))
                .is_some_and(|bytes| img.as_bytes().len() == bytes)
        })
}

fn native_mesh_transform_supported_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    matches!(filter, ResampleFilter::Nearest)
        && width != 0
        && height != 0
        && shape.width != 0
        && shape.height != 0
        && !data.is_empty()
        && data.len() % 12 == 0
        && data.iter().all(|value| value.is_finite())
        && native_affine_nearest_transform_channels_for_shape(shape, mode).is_some()
}

pub(super) fn supports_for_image(
    img: &DynamicImage,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    match method {
        TransformMethod::Affine => {
            native_affine_luma16_transform_supported_for_image(
                img, width, height, method, data, filter, mode,
            ) || native_affine_nearest_transform_supported_for_image(
                img, width, height, method, data, filter, mode,
            )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            native_projective_nearest_transform_supported_for_image(
                img, width, height, method, data, filter, mode,
            )
        }
        TransformMethod::Mesh => {
            native_mesh_transform_supported_for_image(img, width, height, data, filter, mode)
        }
    }
}

pub(super) fn supports_for_shape(
    shape: SimdImageShape,
    width: u32,
    height: u32,
    method: &TransformMethod,
    data: &[f64],
    filter: ResampleFilter,
    mode: Option<&str>,
) -> bool {
    match method {
        TransformMethod::Affine => {
            native_affine_luma16_transform_supported_for_shape(
                shape, width, height, method, data, filter, mode,
            ) || native_affine_nearest_transform_supported_for_shape(
                shape, width, height, method, data, filter, mode,
            )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            native_projective_nearest_transform_supported_for_shape(
                shape, width, height, method, data, filter, mode,
            )
        }
        TransformMethod::Mesh => {
            native_mesh_transform_supported_for_shape(shape, width, height, data, filter, mode)
        }
    }
}

/// Execute the native byte affine subset of `Image.transform`.
/// Scalar coordinate construction and bounds checks form the control plane;
/// every output group is packed and stored through a native SIMD byte block.
pub fn simd_transform(
    img: &DynamicImage,
    op: &PipelineOp,
    mode: Option<&str>,
) -> Result<DynamicImage, PilError> {
    let PipelineOp::Transform {
        w,
        h,
        method,
        data,
        filter,
        fill,
        fill_is_none,
        palette_fill: _,
        ..
    } = op
    else {
        return Err(PilError::ValueError("expected Transform op".into()));
    };
    let luma16_supported = native_affine_luma16_transform_supported_for_image(
        img, *w, *h, method, data, *filter, mode,
    );
    let supported = match method {
        TransformMethod::Affine => {
            luma16_supported
                || native_affine_nearest_transform_supported_for_image(
                    img, *w, *h, method, data, *filter, mode,
                )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            native_projective_nearest_transform_supported_for_image(
                img, *w, *h, method, data, *filter, mode,
            )
        }
        TransformMethod::Mesh => {
            native_mesh_transform_supported_for_image(img, *w, *h, data, *filter, mode)
        }
    };
    if !supported {
        return Err(simd_unsupported("Transform"));
    }
    if luma16_supported {
        return simd_affine_luma16_transform_bytes(img, *w, *h, data, *fill, *filter, mode)?
            .ok_or_else(|| simd_unsupported("Transform"));
    }
    match method {
        TransformMethod::Affine => match filter {
            ResampleFilter::Nearest => {
                simd_affine_nearest_transform_bytes(img, *w, *h, data, *fill, mode)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            ResampleFilter::Bilinear => {
                simd_affine_bilinear_transform_bytes(img, *w, *h, data, *fill, mode)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Perspective => match filter {
            ResampleFilter::Nearest => {
                simd_projective_nearest_transform_bytes(img, *w, *h, data, *fill, mode, false)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            ResampleFilter::Bilinear => {
                simd_projective_bilinear_transform_bytes(img, *w, *h, data, *fill, mode, false)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Quad => match filter {
            ResampleFilter::Nearest => {
                simd_projective_nearest_transform_bytes(img, *w, *h, data, *fill, mode, true)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            ResampleFilter::Bilinear => {
                simd_projective_bilinear_transform_bytes(img, *w, *h, data, *fill, mode, true)?
                    .ok_or_else(|| simd_unsupported("Transform"))
            }
            _ => Err(simd_unsupported("Transform")),
        },
        TransformMethod::Mesh => {
            simd_mesh_transform_bytes(img, *w, *h, data, *fill, *fill_is_none, mode)?
                .ok_or_else(|| simd_unsupported("Transform"))
        }
    }
}
