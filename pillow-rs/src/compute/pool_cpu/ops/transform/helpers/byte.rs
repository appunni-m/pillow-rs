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

pub(crate) fn execute<const CHANNELS: usize, const FORCE_AFFINE_NEAREST: bool, const ALPHA: bool>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
    nearest_kernel: AffineNearestKernel,
) -> Result<Vec<u8>, PilError> {
    match request.method {
        TransformMethod::Affine => execute_affine::<CHANNELS, FORCE_AFFINE_NEAREST, ALPHA>(
            image,
            request,
            nearest_kernel,
        ),
        TransformMethod::Perspective | TransformMethod::Quad => {
            execute_projective::<CHANNELS, ALPHA>(image, request)
        }
        TransformMethod::Mesh => execute_mesh::<CHANNELS, ALPHA>(image, request),
    }
}

fn execute_affine<const CHANNELS: usize, const FORCE_NEAREST: bool, const ALPHA: bool>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
    nearest_kernel: AffineNearestKernel,
) -> Result<Vec<u8>, PilError> {
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
        && let Some(output) = affine_nearest(
            nearest_kernel,
            image.as_bytes(),
            dimensions,
            request,
            affine,
        )?
    {
        return Ok(output);
    }
    let source = if ALPHA && !nearest {
        alpha::premultiply(image.as_bytes(), CHANNELS)
    } else {
        image.as_bytes().to_vec()
    };
    let mut output = affine::transform_affine_generic(
        &source,
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
    )?;
    if ALPHA && !nearest {
        alpha::unpremultiply(&mut output, CHANNELS);
    }
    Ok(output)
}

fn affine_nearest(
    kernel: AffineNearestKernel,
    source: &[u8],
    source_dimensions: (u32, u32),
    request: TransformRequest<'_>,
    affine: [f64; 6],
) -> Result<Option<Vec<u8>>, PilError> {
    match kernel {
        AffineNearestKernel::Generic => Ok(None),
        AffineNearestKernel::Luma => affine::transform_affine_luma_nearest(
            source,
            source_dimensions,
            request.width,
            request.height,
            affine,
            request.fill,
        ),
        AffineNearestKernel::LumaAlpha => affine::transform_affine_luma_alpha_nearest(
            source,
            source_dimensions,
            request.width,
            request.height,
            affine,
            request.fill,
        ),
        AffineNearestKernel::Rgb => affine::transform_affine_rgb_nearest(
            source,
            source_dimensions,
            request.width,
            request.height,
            affine,
            request.fill,
        ),
    }
}

fn execute_projective<const CHANNELS: usize, const ALPHA: bool>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
) -> Result<Vec<u8>, PilError> {
    let nearest = matches!(request.filter, ResampleFilter::Nearest);
    let source = if ALPHA && !nearest {
        alpha::premultiply(image.as_bytes(), CHANNELS)
    } else {
        image.as_bytes().to_vec()
    };
    let mut output = projective::transform_projective_generic(
        &source,
        image.dimensions(),
        request.width,
        request.height,
        request.data,
        &request.filter,
        request.fill,
        CHANNELS,
        matches!(request.method, TransformMethod::Quad),
    )?;
    if ALPHA && !nearest {
        alpha::unpremultiply(&mut output, CHANNELS);
    }
    Ok(output)
}

fn execute_mesh<const CHANNELS: usize, const ALPHA: bool>(
    image: &DynamicImage,
    request: TransformRequest<'_>,
) -> Result<Vec<u8>, PilError> {
    let nearest = matches!(request.filter, ResampleFilter::Nearest);
    let source = if ALPHA && !nearest {
        alpha::premultiply(image.as_bytes(), CHANNELS)
    } else {
        image.as_bytes().to_vec()
    };
    let mut output = projective::transform_mesh_with_filter(
        &source,
        image.dimensions(),
        request.width,
        request.height,
        request.data,
        request.fill,
        CHANNELS,
        request.filter,
        request.fill_is_none,
    )?;
    if ALPHA && !nearest {
        alpha::unpremultiply(&mut output, CHANNELS);
    }
    Ok(output)
}
