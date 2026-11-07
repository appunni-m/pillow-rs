//! Shared arithmetic for native-byte Blend mode strategies.

use crate::raster::GenericImageView;

use crate::checked_dims::CheckedDims;
use crate::compute::Architecture;
use crate::compute::pool_cpu::helpers::bytes::hidden_lanes::default_hidden_lanes_for_bytes;
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, Mode, RgbImage};

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) mod avx_fma;
#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
pub(crate) mod neon;

pub(crate) fn execute(
    source: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    alpha: f64,
    channels: u8,
) -> Result<DynamicImage, PilError> {
    let (width, height) = (source.width(), source.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
    let source_bytes = source.as_bytes();
    let other_bytes = other.as_bytes();
    if source.mode() != mode
        || source.dimensions() != other.dimensions()
        || source_bytes.len() != dimensions.total_bytes()
        || other_bytes.len() != dimensions.total_bytes()
    {
        return Err(PilError::ValueError("images do not match".into()));
    }
    blend_bytes(
        width,
        height,
        mode,
        alpha,
        source_bytes,
        other_bytes,
        mode == Mode::ImageLab8,
        other.mode() == Mode::ImageLab8,
    )
}

/// Blend the base and input LAB rasters through the portable byte strategy.
pub(crate) fn execute_lab_samples(
    base: &RgbImage,
    source: &RgbImage,
    alpha: f64,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::strategy::require_operation_mode(selection, OperationId::BlendModule, Mode::ImageLab8)?;
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError(
            "LAB Blend selected an undeclared architecture".into(),
        ));
    }
    if source.dimensions() != base.dimensions() {
        return Err(PilError::ValueError("images do not match".into()));
    }
    let dimensions = CheckedDims::new_allow_empty(base.width(), base.height(), 3)?;
    if source.as_raw().len() != dimensions.total_bytes()
        || base.as_raw().len() != dimensions.total_bytes()
    {
        return Err(PilError::ValueError("images do not match".into()));
    }
    let mut output = blend_bytes(
        base.width(),
        base.height(),
        Mode::ImageLab8,
        alpha,
        base.as_raw(),
        source.as_raw(),
        true,
        true,
    )?;
    let dimensions = (base.width() as usize, base.height() as usize);
    let lanes = blend_hidden_lanes(
        Mode::ImageLab8,
        base.hidden_pixel_lanes(),
        source.hidden_pixel_lanes(),
        base.as_raw(),
        source.as_raw(),
        dimensions,
        alpha as f32,
    )?;
    if !output.replace_hidden_pixel_lanes(lanes) {
        return Err(PilError::InternalError(
            "LAB Blend output physical lanes did not match its mode".into(),
        ));
    }
    Ok(output)
}

fn attach_hidden_lanes(
    source: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    alpha: f32,
    output: &mut DynamicImage,
) -> Result<(), PilError> {
    let dimensions = (source.width() as usize, source.height() as usize);
    let lanes = blend_hidden_lanes(
        mode,
        source.hidden_pixel_lanes(),
        other.hidden_pixel_lanes(),
        source.as_bytes(),
        other.as_bytes(),
        dimensions,
        alpha,
    )?;
    if !output.replace_hidden_pixel_lanes(lanes) {
        return Err(PilError::InternalError(
            "Blend output physical lanes did not match its mode".into(),
        ));
    }
    Ok(())
}

/// Mirrors the physical-row blend for bytes omitted from a compact carrier.
fn blend_hidden_lanes(
    mode: Mode,
    left: Option<&HiddenPixelLanes>,
    right: Option<&HiddenPixelLanes>,
    left_bytes: &[u8],
    right_bytes: &[u8],
    dimensions: (usize, usize),
    alpha: f32,
) -> Result<Option<HiddenPixelLanes>, PilError> {
    let Some(lane_count) = mode.hidden_pixel_lane_count() else {
        return Ok(None);
    };
    let pixel_count = dimensions
        .0
        .checked_mul(dimensions.1)
        .ok_or_else(|| PilError::DimensionError("Blend lane extent overflow".into()))?;
    if let Some(lanes) = left.filter(|lanes| {
        lanes.lane_count() != lane_count || lanes.pixel_count() != pixel_count
    }) {
        return Err(PilError::InternalError(format!(
            "Blend source hidden lane shape is {} lanes across {} pixels, expected {lane_count} lanes across {pixel_count} pixels",
            lanes.lane_count(),
            lanes.pixel_count(),
        )));
    }
    if let Some(lanes) = right.filter(|lanes| {
        lanes.lane_count() != lane_count || lanes.pixel_count() != pixel_count
    }) {
        return Err(PilError::InternalError(format!(
            "Blend input hidden lane shape is {} lanes across {} pixels, expected {lane_count} lanes across {pixel_count} pixels",
            lanes.lane_count(),
            lanes.pixel_count(),
        )));
    }
    if alpha == 0.0 {
        return Ok(left.cloned());
    }
    if alpha == 1.0 {
        return Ok(right.cloned());
    }
    if pixel_count == 0 || (left.is_none() && right.is_none()) {
        return Ok(None);
    }
    let left_fallback = if left.is_none() {
        default_hidden_lanes_for_bytes(mode, left_bytes, pixel_count)?
    } else {
        None
    };
    let right_fallback = if right.is_none() {
        default_hidden_lanes_for_bytes(mode, right_bytes, pixel_count)?
    } else {
        None
    };
    let left = left.or(left_fallback.as_ref()).ok_or_else(|| {
        PilError::InternalError("Blend omitted left physical-lane state".into())
    })?;
    let right = right.or(right_fallback.as_ref()).ok_or_else(|| {
        PilError::InternalError("Blend omitted right physical-lane state".into())
    })?;
    left.zip_map_clipped(
        right,
        dimensions,
        dimensions,
        dimensions,
        |left, right| blend_raw_sample(left, right, alpha),
    )
    .map(Some)
    .ok_or_else(|| PilError::InternalError("Blend physical lane extents did not match".into()))
}

fn blend_bytes(
    width: u32,
    height: u32,
    mode: Mode,
    alpha: f64,
    source_bytes: &[u8],
    other_bytes: &[u8],
    first_lab: bool,
    second_lab: bool,
) -> Result<DynamicImage, PilError> {
    let alpha = alpha as f32;
    let output = source_bytes
        .iter()
        .zip(other_bytes)
        .enumerate()
        .map(|(index, (&left, &right))| {
            blend_sample(left, right, index, first_lab, second_lab, alpha)
        })
        .collect();
    crate::compute::pool_cpu::ops::composite::helpers::native_bytes::construct(
        width, height, mode, output,
    )
}

#[inline]
fn blend_sample(
    left: u8,
    right: u8,
    index: usize,
    first_lab: bool,
    second_lab: bool,
    alpha: f32,
) -> u8 {
    let bias = if index % 3 == 0 { 0 } else { 128 };
    let left = left ^ if first_lab { bias } else { 0 };
    let right = right ^ if second_lab { bias } else { 0 };
    blend_raw_sample(left, right, alpha) ^ if first_lab { bias } else { 0 }
}

#[inline]
pub(super) fn blend_raw_sample(left: u8, right: u8, alpha: f32) -> u8 {
    alpha
        .mul_add(f32::from(right) - f32::from(left), f32::from(left))
        .clamp(0.0, 255.0) as u8
}

/// Executes the archived sixteen-byte FMA loop with the selected arch kernel.
/// The closure is statically dispatched and monomorphized into this loop.
#[inline(always)]
pub(super) fn execute_vector<F>(
    source: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    channels: u8,
    alpha: f64,
    mut blend_block: F,
) -> Result<DynamicImage, PilError>
where
    F: FnMut([u8; 16], [u8; 16], f32) -> [u8; 16],
{
    let (width, height) = (source.width(), source.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
    let source_bytes = source.as_bytes();
    let other_bytes = other.as_bytes();
    let alpha = alpha as f32;
    if source.mode() != mode
        || source_bytes.len() != dimensions.total_bytes()
        || other_bytes.len() != dimensions.total_bytes()
        || !alpha.is_finite()
    {
        return Err(PilError::InternalError(
            "native Blend strategy received an invalid prepared layout".into(),
        ));
    }
    if source_bytes.is_empty() {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(source.clone());
    }

    let mut output = vec![0u8; source_bytes.len()];
    let mut source_chunks = source_bytes.chunks_exact(16);
    let mut other_chunks = other_bytes.chunks_exact(16);
    let mut output_chunks = output.chunks_exact_mut(16);
    for ((source, other), output) in source_chunks
        .by_ref()
        .zip(other_chunks.by_ref())
        .zip(output_chunks.by_ref())
    {
        let source: [u8; 16] = source.try_into().expect("complete vector");
        let other: [u8; 16] = other.try_into().expect("complete vector");
        output.copy_from_slice(&blend_block(source, other, alpha));
    }

    let remainder = output_chunks.into_remainder();
    let mut vector_blocks = source_bytes.len() / 16;
    let scalar_tail = if remainder.len() >= 8 {
        let mut source_block = [0u8; 16];
        let mut other_block = [0u8; 16];
        source_block[..remainder.len()].copy_from_slice(source_chunks.remainder());
        other_block[..remainder.len()].copy_from_slice(other_chunks.remainder());
        remainder.copy_from_slice(&blend_block(source_block, other_block, alpha)[..remainder.len()]);
        vector_blocks += 1;
        0
    } else {
        for ((output, &source), &other) in remainder
            .iter_mut()
            .zip(source_chunks.remainder())
            .zip(other_chunks.remainder())
        {
            *output = blend_raw_sample(source, other, alpha);
        }
        remainder.len()
    };
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks as u64);
    crate::compute::record_pipeline_operation_scalar_tail(scalar_tail as u64);
    crate::compute::record_pipeline_operation_path("vector");
    crate::compute::pool_cpu::ops::composite::helpers::native_bytes::construct(
        width, height, mode, output,
    )
}

pub(crate) fn execute_selected(
    source: &DynamicImage,
    other: &DynamicImage,
    mode: Mode,
    channels: u8,
    alpha: f64,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::strategy::require_operation_mode(selection, OperationId::BlendModule, mode)?;
    if source.mode() != mode || source.dimensions() != other.dimensions()
    {
        return Err(PilError::ValueError("images do not match".into()));
    }
    let mut output = match selection.implementation.architecture {
        Architecture::Default => execute(source, other, mode, alpha, channels),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Architecture::AvxFma if vector_mode(mode) => {
            avx_fma::execute(source, other, mode, channels, alpha)
        }
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon if vector_mode(mode) => {
            neon::execute(source, other, mode, channels, alpha)
        }
        architecture => Err(PilError::InternalError(format!(
            "unbound {architecture:?} Blend strategy for mode {mode:?}"
        ))),
    }?;
    attach_hidden_lanes(source, other, mode, alpha as f32, &mut output)?;
    Ok(output)
}

fn vector_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::ImageLuma8
            | Mode::ImageLumaA8
            | Mode::ImageLaPremultiplied8
            | Mode::ImageRgb8
            | Mode::ImageHsv8
            | Mode::ImageYCbCr8
            | Mode::ImageRgba8
            | Mode::ImageCmyk8
            | Mode::ImageRgbPremultiplied8
            | Mode::ImageRgbx8
    )
}
