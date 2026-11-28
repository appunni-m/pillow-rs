//! Default Transform implementation for exact mode ImageLab8.

use crate::compute::pool_cpu::ops::transform::helpers::{byte, projective};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::raster::{DynamicImage, GenericImageView, RgbImage};

/// Pillow decodes LAB byte data into its centered chroma representation before
/// Geometry filters it, then removes the 128 bias when it emits bytes. Keep
/// that mode boundary explicit around the shared sampler.
// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
pub(crate) fn execute(
    input: &DynamicImage,
    request: TransformRequest<'_>,
) -> Result<DynamicImage, PilError> {
    let biased_input = biased_source(input)?;
    let mut output = byte::execute::<3, false, false>(
        &biased_input,
        request,
        byte::AffineNearestKernel::Generic,
    )?;
    remove_lab_chroma_bias(&mut output);
    let image = RgbImage::from_raw(request.width, request.height, output).ok_or_else(|| {
        PilError::InternalError("transform ImageLab8 output buffer shape mismatch".into())
    })?;
    Ok(DynamicImage::ImageLab8(image))
}

// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
pub(crate) fn execute_into(
    input: &DynamicImage,
    request: TransformRequest<'_>,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    let biased_input = biased_source(input)?;
    let destination = lab_output_bytes_mut(output)?;
    byte::execute_into::<3, false, false>(
        &biased_input,
        request,
        byte::AffineNearestKernel::Generic,
        destination,
    )?;
    remove_lab_chroma_bias(destination);
    Ok(())
}

// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
pub(crate) fn execute_into_with_validity(
    input: &DynamicImage,
    request: TransformRequest<'_>,
    output: &mut DynamicImage,
    validity: &mut [u8],
) -> Result<(), PilError> {
    let biased_input = biased_source(input)?;
    let destination = lab_output_bytes_mut(output)?;
    byte::execute_into_with_validity::<3, false, false>(
        &biased_input,
        request,
        byte::AffineNearestKernel::Generic,
        destination,
        Some(validity),
    )?;
    remove_lab_chroma_bias(destination);
    Ok(())
}

// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
pub(crate) fn apply_mesh_record(
    input: &DynamicImage,
    output: &mut DynamicImage,
    request: TransformRequest<'_>,
    box_coordinates: (i64, i64, i64, i64),
    coefficients: [f64; 8],
    source_scratch: &mut Vec<u8>,
) -> Result<(), PilError> {
    if !matches!(input, DynamicImage::ImageLab8(_)) {
        return Err(PilError::InternalError(
            "Transform ImageLab8 source mode mismatch".into(),
        ));
    }
    let dimensions = input.dimensions();
    source_scratch.clear();
    source_scratch.extend_from_slice(input.as_bytes());
    add_lab_chroma_bias(source_scratch);
    projective::transform_mesh_record_with_coefficients(
        source_scratch,
        dimensions,
        (request.width, request.height),
        box_coordinates,
        coefficients,
        lab_output_bytes_mut(output)?,
        3,
        request.filter,
        request.fill_is_none,
    )
}

// VERIFY: source=reviewed@2026-11-26; parity=pending; benchmark=pending
pub(crate) fn remove_output_chroma_bias(output: &mut DynamicImage) -> Result<(), PilError> {
    remove_lab_chroma_bias(lab_output_bytes_mut(output)?);
    Ok(())
}

fn biased_source(input: &DynamicImage) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageLab8(image) = input else {
        return Err(PilError::InternalError(
            "Transform ImageLab8 strategy received a different mode".into(),
        ));
    };
    let mut bytes = image.as_raw().clone();
    add_lab_chroma_bias(&mut bytes);
    let image = RgbImage::from_raw(image.width(), image.height(), bytes).ok_or_else(|| {
        PilError::InternalError("Transform ImageLab8 source buffer shape mismatch".into())
    })?;
    Ok(DynamicImage::ImageLab8(image))
}

fn lab_output_bytes_mut(output: &mut DynamicImage) -> Result<&mut [u8], PilError> {
    let DynamicImage::ImageLab8(image) = output else {
        return Err(PilError::InternalError(
            "Transform ImageLab8 destination mode mismatch".into(),
        ));
    };
    Ok(image.as_mut())
}

fn add_lab_chroma_bias(bytes: &mut [u8]) {
    for pixel in bytes.chunks_exact_mut(3) {
        pixel[1] = pixel[1].wrapping_add(128);
        pixel[2] = pixel[2].wrapping_add(128);
    }
}

fn remove_lab_chroma_bias(bytes: &mut [u8]) {
    for pixel in bytes.chunks_exact_mut(3) {
        pixel[1] = pixel[1].wrapping_sub(128);
        pixel[2] = pixel[2].wrapping_sub(128);
    }
}
