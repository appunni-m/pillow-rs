//! Pillow physical-byte behavior for compact-carrier Transform modes.

use super::{affine, projective, vector_affine_nearest, vector_projective_nearest};
use crate::compute::{Architecture, Backend};
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::compute::pool_cpu::ops::transform::ops::TransformRequest;
use crate::error::PilError;
use crate::pipeline::{ResampleFilter, TransformMethod};
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, GenericImageView, Mode};

/// Restores the physical lanes omitted from LA/PA and three-band byte carriers.
///
/// Pillow's nearest filter copies complete four-byte pixels. Its filtered
/// two-band callbacks write the first result byte to physical lanes 0, 1, and
/// 2, while the straight-LA wrapper converts back from `La` and repeats luma.
/// Its filtered three-band callbacks leave lane 3 at the output image's
/// zero-filled or explicit-fill value.
pub(crate) fn finish(
    source: &DynamicImage,
    output: &mut DynamicImage,
    request: TransformRequest<'_>,
    selection: &StrategySelection,
) -> Result<(), PilError> {
    let mode = source.mode();
    validate_selection(selection, mode)?;
    if output.mode() != mode {
        return Err(PilError::InternalError(
            "Transform output changed its source mode".into(),
        ));
    }
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(());
    };
    let pixel_count = pixel_count(output)?;
    let lanes = match mode {
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => {
            if matches!(request.filter, ResampleFilter::Nearest) {
                match source.hidden_pixel_lanes() {
                    Some(_) => sample_source_lanes::<2>(source, mode, request, selection)?,
                    None if two_band_fill_matches_luma(mode, request) => {
                        repeat_first_sample(output, pixel_count, lane_count)?
                    }
                    None => sample_source_lanes::<2>(source, mode, request, selection)?,
                }
            } else {
                repeat_first_sample(output, pixel_count, lane_count)?
            }
        }
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => {
            if matches!(request.filter, ResampleFilter::Nearest) {
                sample_source_lanes::<1>(source, mode, request, selection)?
            } else {
                let fill = hidden_fill(mode, request)[0];
                HiddenPixelLanes::constant(lane_count, pixel_count, [fill, 0])
                    .ok_or_else(invalid_lanes)?
            }
        }
        _ => return Ok(()),
    };
    if !output.replace_hidden_pixel_lanes(Some(lanes)) {
        return Err(invalid_lanes());
    }
    Ok(())
}

// A missing source sidecar is recoverable from luma except when a nearest
// transform's explicit packed fill carries different physical bytes.
fn two_band_fill_matches_luma(mode: Mode, request: TransformRequest<'_>) -> bool {
    let luma = request.fill.map_or(0, |fill| fill.0);
    hidden_fill(mode, request) == [luma, luma]
}

fn sample_source_lanes<const CHANNELS: usize>(
    source: &DynamicImage,
    mode: Mode,
    request: TransformRequest<'_>,
    selection: &StrategySelection,
) -> Result<HiddenPixelLanes, PilError> {
    let architecture = selection.implementation.architecture;
    let raw = source_lane_plane(source, mode, CHANNELS)?;
    let fill = hidden_fill(mode, request);
    let fill = [fill[0], fill[1], 0, 0];
    let bytes = if architecture == Architecture::Default
        || matches!(request.method, TransformMethod::Mesh)
    {
        sample_default::<CHANNELS>(source, request, &raw, fill)?
    } else {
        sample_vector::<CHANNELS>(source, request, &raw, fill)?
    };
    HiddenPixelLanes::per_pixel(CHANNELS, pixel_count_from_request(request)?, bytes)
        .ok_or_else(invalid_lanes)
}

fn validate_selection(
    selection: &StrategySelection,
    mode: Mode,
) -> Result<(), PilError> {
    let implementation = selection.implementation;
    if implementation.operation != OperationId::Transform
        || implementation.mode != mode
        || implementation.backend != Backend::Cpu
        || selection.requested_backend != Backend::Cpu
    {
        return Err(PilError::InternalError(
            "Transform hidden-lane helper received a mismatched strategy selection".into(),
        ));
    }
    Ok(())
}

fn source_lane_plane(
    source: &DynamicImage,
    mode: Mode,
    lane_count: usize,
) -> Result<Vec<u8>, PilError> {
    let pixel_count = pixel_count(source)?;
    let output_len = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(invalid_lanes)?;
    let carrier_width = mode
        .byte_carrier_pixel_width()
        .ok_or_else(invalid_lanes)?;
    let logical = source.as_bytes();
    if logical.len() < pixel_count.checked_mul(carrier_width).ok_or_else(invalid_lanes)? {
        return Err(invalid_lanes());
    }
    let retained = source.hidden_pixel_lanes();
    if retained.is_some_and(|lanes| {
        lanes.lane_count() != lane_count || lanes.pixel_count() != pixel_count
    }) {
        return Err(invalid_lanes());
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_len)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for index in 0..pixel_count {
        if let Some(lanes) = retained {
            output.extend_from_slice(lanes.for_pixel(index).ok_or_else(invalid_lanes)?);
        } else {
            let fallback = match carrier_width {
                2 => [logical[index * carrier_width], logical[index * carrier_width]],
                3 => [u8::MAX, 0],
                _ => return Err(invalid_lanes()),
            };
            output.extend_from_slice(&fallback[..lane_count]);
        }
    }
    Ok(output)
}

fn sample_default<const CHANNELS: usize>(
    source: &DynamicImage,
    request: TransformRequest<'_>,
    raw: &[u8],
    fill: [u8; 4],
) -> Result<Vec<u8>, PilError> {
    match request.method {
        TransformMethod::Affine => {
            let [a, b, c, d, e, f] = request.data else {
                return Err(invalid_request());
            };
            affine::transform_affine_generic(
                raw,
                source.dimensions(),
                request.width,
                request.height,
                *a,
                *b,
                *c,
                *d,
                *e,
                *f,
                Some((fill[0], fill[1], 0, 0)),
                CHANNELS,
                true,
                false,
            )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            projective::transform_projective_generic(
                raw,
                source.dimensions(),
                request.width,
                request.height,
                request.data,
                &request.filter,
                Some((fill[0], fill[1], 0, 0)),
                CHANNELS,
                matches!(request.method, TransformMethod::Quad),
            )
        }
        TransformMethod::Mesh => projective::transform_mesh_with_filter(
            raw,
            source.dimensions(),
            request.width,
            request.height,
            request.data,
            Some((fill[0], fill[1], 0, 0)),
            CHANNELS,
            request.filter,
            request.fill_is_none,
        ),
    }
}

fn sample_vector<const CHANNELS: usize>(
    source: &DynamicImage,
    request: TransformRequest<'_>,
    raw: &[u8],
    fill: [u8; 4],
) -> Result<Vec<u8>, PilError> {
    let destination = (request.width, request.height);
    match request.method {
        TransformMethod::Affine => {
            let [a, b, c, d, e, f] = request.data else {
                return Err(invalid_request());
            };
            vector_affine_nearest::execute::<CHANNELS>(
                raw,
                source.dimensions(),
                destination,
                [*a, *b, *c, *d, *e, *f],
                fill,
            )
        }
        TransformMethod::Perspective | TransformMethod::Quad => {
            vector_projective_nearest::execute::<CHANNELS>(
                raw,
                source.dimensions(),
                destination,
                request.data,
                fill,
                matches!(request.method, TransformMethod::Quad),
            )
        }
        TransformMethod::Mesh => Err(PilError::InternalError(
            "Transform mesh cannot select a vector architecture".into(),
        )),
    }
}

fn repeat_first_sample(
    image: &DynamicImage,
    pixel_count: usize,
    lane_count: usize,
) -> Result<HiddenPixelLanes, PilError> {
    let carrier_width = image
        .mode()
        .byte_carrier_pixel_width()
        .ok_or_else(invalid_lanes)?;
    let logical = image.as_bytes();
    if logical.len() < pixel_count.checked_mul(carrier_width).ok_or_else(invalid_lanes)? {
        return Err(invalid_lanes());
    }
    let value_count = pixel_count
        .checked_mul(lane_count)
        .ok_or_else(invalid_lanes)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(value_count)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    for pixel in logical.chunks_exact(carrier_width).take(pixel_count) {
        for _ in 0..lane_count {
            values.push(pixel[0]);
        }
    }
    HiddenPixelLanes::per_pixel(lane_count, pixel_count, values).ok_or_else(invalid_lanes)
}

fn hidden_fill(mode: Mode, request: TransformRequest<'_>) -> [u8; 2] {
    if request.fill_is_none {
        return [0, 0];
    }
    // public_transform_fill stores LA-family components as (L, A, lane1,
    // lane2); RGB-family component four is the omitted physical padding byte.
    request.fill.map_or([0, 0], |fill| {
        if mode.hidden_pixel_lane_count() == Some(1) {
            [fill.3, 0]
        } else {
            [fill.2, fill.3]
        }
    })
}

fn pixel_count(image: &DynamicImage) -> Result<usize, PilError> {
    usize::try_from(image.width())
        .ok()
        .and_then(|width| {
            usize::try_from(image.height())
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(invalid_lanes)
}

fn pixel_count_from_request(request: TransformRequest<'_>) -> Result<usize, PilError> {
    usize::try_from(request.width)
        .ok()
        .and_then(|width| {
            usize::try_from(request.height)
                .ok()
                .and_then(|height| width.checked_mul(height))
        })
        .ok_or_else(invalid_lanes)
}

fn invalid_request() -> PilError {
    PilError::InternalError("Transform coefficients changed after admission".into())
}

fn invalid_lanes() -> PilError {
    PilError::InternalError("Transform physical lane output was invalid".into())
}
