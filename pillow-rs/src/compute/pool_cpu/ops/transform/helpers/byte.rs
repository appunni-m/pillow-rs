//! Shared byte sampling used by the exact byte-mode Transform strategies.

use crate::compute::pool_cpu::ops::transform::helpers::{affine, alpha, projective};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, GenericImageView};

#[derive(Clone, Copy)]
pub(crate) enum AffineNearestKernel {
    Generic,
    Luma,
    LumaAlpha,
    Rgb,
}

pub(crate) fn execute<
    const CHANNELS: usize,
    const FORCE_AFFINE_NEAREST: bool,
    const ALPHA: bool,
>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
    nearest_kernel: AffineNearestKernel,
) -> Result<Vec<u8>, PilError> {
    let output_len = usize::try_from(request.width)
        .ok()
        .and_then(|width| {
            usize::try_from(request.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    let mut output = vec![0; output_len];
    execute_into::<CHANNELS, FORCE_AFFINE_NEAREST, ALPHA>(
        image,
        request,
        nearest_kernel,
        &mut output,
    )?;
    Ok(output)
}

pub(crate) fn execute_into<
    const CHANNELS: usize,
    const FORCE_AFFINE_NEAREST: bool,
    const ALPHA: bool,
>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
    nearest_kernel: AffineNearestKernel,
    output: &mut [u8],
) -> Result<(), PilError> {
    let expected = usize::try_from(request.width)
        .ok()
        .and_then(|width| {
            usize::try_from(request.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .and_then(|pixels| pixels.checked_mul(CHANNELS))
        .ok_or_else(|| PilError::ValueError("image dimensions are too large".into()))?;
    if output.len() != expected {
        return Err(PilError::InternalError(
            "Transform byte destination buffer shape mismatch".into(),
        ));
    }
    match request.method {
        TransformMethod::Affine => execute_affine_into::<CHANNELS, FORCE_AFFINE_NEAREST, ALPHA>(
            image,
            request,
            nearest_kernel,
            output,
        ),
        TransformMethod::Perspective | TransformMethod::Quad => {
            execute_projective_into::<CHANNELS, ALPHA>(image, request, output)
        }
        TransformMethod::Mesh => execute_mesh_into::<CHANNELS, ALPHA>(image, request, output),
    }
}

fn execute_affine_into<const CHANNELS: usize, const FORCE_NEAREST: bool, const ALPHA: bool>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
    nearest_kernel: AffineNearestKernel,
    output: &mut [u8],
) -> Result<(), PilError> {
    let dimensions = image.dimensions();
    let affine = [
        request.data[0],
        request.data[1],
        request.data[2],
        request.data[3],
        request.data[4],
        request.data[5],
    ];
    let nearest = FORCE_NEAREST || matches!(request.filter, ResampleFilter::Nearest);
    if nearest
        && affine_nearest_into(
            nearest_kernel,
            image.as_bytes(),
            dimensions,
            request,
            affine,
            output,
        )?
    {
        return Ok(());
    }
    let premultiplied;
    let source = if ALPHA && !nearest {
        premultiplied = alpha::premultiply(image.as_bytes(), CHANNELS);
        premultiplied.as_slice()
    } else {
        image.as_bytes()
    };
    affine::transform_affine_generic_into(
        source,
        dimensions,
        request.width,
        request.height,
        affine[0],
        affine[1],
        affine[2],
        affine[3],
        affine[4],
        affine[5],
        request.fill,
        CHANNELS,
        nearest,
        matches!(request.filter, ResampleFilter::Bicubic),
        output,
    )?;
    if ALPHA && !nearest {
        alpha::unpremultiply(output, CHANNELS);
    }
    Ok(())
}

fn affine_nearest_into(
    kernel: AffineNearestKernel,
    source: &[u8],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    affine: [f64; 6],
    output: &mut [u8],
) -> Result<bool, PilError> {
    match kernel {
        AffineNearestKernel::Generic => Ok(false),
        AffineNearestKernel::Luma => affine::transform_affine_luma_nearest_into(
            source,
            source_dimensions,
            request.width,
            request.height,
            affine,
            request.fill,
            output,
        ),
        AffineNearestKernel::LumaAlpha => affine::transform_affine_luma_alpha_nearest_into(
            source,
            source_dimensions,
            request.width,
            request.height,
            affine,
            request.fill,
            output,
        ),
        AffineNearestKernel::Rgb => affine::transform_affine_rgb_nearest_into(
            source,
            source_dimensions,
            request.width,
            request.height,
            affine,
            request.fill,
            output,
        ),
    }
}

fn execute_projective_into<const CHANNELS: usize, const ALPHA: bool>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
    output: &mut [u8],
) -> Result<(), PilError> {
    let nearest = matches!(request.filter, ResampleFilter::Nearest);
    let premultiplied;
    let source = if ALPHA && !nearest {
        premultiplied = alpha::premultiply(image.as_bytes(), CHANNELS);
        premultiplied.as_slice()
    } else {
        image.as_bytes()
    };
    projective::transform_projective_generic_into(
        source,
        image.dimensions(),
        request.width,
        request.height,
        request.data,
        &request.filter,
        request.fill,
        CHANNELS,
        matches!(request.method, TransformMethod::Quad),
        output,
    )?;
    if ALPHA && !nearest {
        alpha::unpremultiply(output, CHANNELS);
    }
    Ok(())
}

fn execute_mesh_into<const CHANNELS: usize, const ALPHA: bool>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
    output: &mut [u8],
) -> Result<(), PilError> {
    let nearest = matches!(request.filter, ResampleFilter::Nearest);
    let premultiplied;
    let source = if ALPHA && !nearest {
        premultiplied = alpha::premultiply(image.as_bytes(), CHANNELS);
        premultiplied.as_slice()
    } else {
        image.as_bytes()
    };
    projective::transform_mesh_with_filter_into(
        source,
        image.dimensions(),
        request.width,
        request.height,
        request.data,
        request.fill,
        CHANNELS,
        request.filter,
        request.fill_is_none,
        output,
    )?;
    if ALPHA && !nearest {
        alpha::unpremultiply(output, CHANNELS);
    }
    Ok(())
}

pub(crate) fn apply_mesh_record<const CHANNELS: usize, const ALPHA: bool>(
    raw: &[u8],
    source_dimensions: (u32, u32),
    output: &mut [u8],
    request: TransformRequest<'_>,
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    source_scratch: &mut Vec<u8>,
) -> Result<(), PilError> {
    let source = if ALPHA && !matches!(request.filter, ResampleFilter::Nearest) {
        alpha::premultiply_into(raw, CHANNELS, source_scratch);
        source_scratch.as_slice()
    } else {
        raw
    };
    projective::transform_mesh_record_with_coefficients(
        source,
        source_dimensions,
        (request.width, request.height),
        box_coordinates,
        coefficients,
        output,
        CHANNELS,
        request.filter,
        request.fill_is_none,
    )
}
