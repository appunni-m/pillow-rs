//! Native sample helpers shared by the exact-mode GetData strategies.

use crate::compute::Architecture;
use crate::compute::registry::support::{
    OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) type Luma16Image =
    crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>;
pub(crate) type LumaA16Image =
    crate::raster::ImageBuffer<crate::raster::LumaA<u16>, Vec<u16>>;
pub(crate) type Rgb16Image =
    crate::raster::ImageBuffer<crate::raster::Rgb<u16>, Vec<u16>>;
pub(crate) type Rgba16Image =
    crate::raster::ImageBuffer<crate::raster::Rgba<u16>, Vec<u16>>;

/// Request dispatched through one exact mode's default strategy.
pub(crate) enum GetDataRequest<'a> {
    /// Return the mode's flat byte representation.
    Full,
    /// Write one logical band into caller-owned storage.
    Band {
        /// Validated zero-based band index.
        index: usize,
        /// One output byte per pixel.
        output: &'a mut [u8],
    },
}

/// Result of executing a direct GetData strategy.
pub(crate) enum GetDataOutput {
    /// Newly allocated full image bytes.
    Bytes(Vec<u8>),
    /// A requested band was written to the provided slice.
    BandWritten,
}

/// Validate the selected operation and resolve its architecture for a mode owner.
pub(crate) fn selected_architecture(
    selection: StrategySelection,
    mode: Mode,
    request: &GetDataRequest<'_>,
) -> Result<Architecture, PilError> {
    let implementation = selection.implementation;
    let operation = match request {
        GetDataRequest::Full if implementation.operation == OperationId::GetData => {
            OperationId::GetData
        }
        GetDataRequest::Band { .. }
            if matches!(
                implementation.operation,
                OperationId::GetDataBand | OperationId::GetDataBandInto
            ) =>
        {
            implementation.operation
        }
        _ => {
            return Err(PilError::InternalError(
                "GetData strategy selection did not match its operation and mode".into(),
            ));
        }
    };
    selected_operation_architecture(selection, mode, operation)
}

/// Validate an exact GetData operation selection without borrowing an output buffer.
pub(crate) fn selected_operation_architecture(
    selection: StrategySelection,
    mode: Mode,
    operation: OperationId,
) -> Result<Architecture, PilError> {
    let implementation = selection.implementation;
    if implementation.mode != mode || implementation.operation != operation {
        return Err(PilError::InternalError(
            "GetData strategy selection did not match its operation and mode".into(),
        ));
    }
    Ok(implementation.architecture)
}

/// Return the error for an architecture without a GetData specialization.
pub(crate) fn missing_architecture(mode: Mode, architecture: Architecture) -> PilError {
    PilError::InternalError(format!(
        "GetData has no {mode:?} {architecture:?} strategy"
    ))
}

/// Return the error for a mode tag whose native sample buffer is malformed.
pub(crate) fn mode_carrier_mismatch(mode: Mode) -> PilError {
    PilError::InternalError(format!(
        "GetData mode {mode:?} does not match its native raster carrier"
    ))
}

/// Compute the exact number of stored samples for a native channel layout.
pub(crate) fn native_sample_count(
    width: u32,
    height: u32,
    channels: u8,
) -> Result<usize, PilError> {
    Ok(crate::checked_dims::CheckedDims::new_allow_empty(width, height, channels)?.total_bytes())
}

/// Narrow an internal native sample buffer using the mode's existing conversion.
pub(crate) fn narrow_native_samples<T: Copy>(
    samples: &[T],
    width: u32,
    height: u32,
    channels: u8,
    mode: Mode,
    convert: impl Fn(T) -> u8,
) -> Result<Vec<u8>, PilError> {
    let sample_count = native_sample_count(width, height, channels)?;
    let samples = samples
        .get(..sample_count)
        .ok_or_else(|| mode_carrier_mismatch(mode))?;
    Ok(samples.iter().copied().map(convert).collect())
}

/// Preserve Pillow's clipped byte view of one native unsigned 16-bit band.
pub(crate) fn narrow_luma16_samples(
    samples: &[u16],
    width: u32,
    height: u32,
    mode: Mode,
) -> Result<Vec<u8>, PilError> {
    narrow_native_samples(samples, width, height, 1, mode, |sample| {
        sample.min(u16::from(u8::MAX)) as u8
    })
}

/// Copy one channel from tightly packed native bytes into a pre-sized output.
#[inline]
pub(crate) fn write_interleaved_band(
    samples: &[u8],
    channels: usize,
    band: usize,
    output: &mut [u8],
) {
    for (destination, source) in output
        .iter_mut()
        .zip(samples.iter().skip(band).step_by(channels))
    {
        *destination = *source;
    }
}

/// Copy one LAB channel while preserving Pillow's signed-byte a/b mapping.
#[inline]
pub(crate) fn write_lab_band(samples: &[u8], band: usize, output: &mut [u8]) {
    for (destination, pixel) in output.iter_mut().zip(samples.chunks_exact(3)) {
        *destination = match band {
            0 => pixel[0],
            1 => pixel[1].wrapping_sub(128),
            2 => pixel[2].wrapping_sub(128),
            _ => unreachable!("LAB has exactly three bands"),
        };
    }
}

/// Write one band from interleaved native samples without changing storage mode.
#[inline]
pub(crate) fn write_native_sample_band<T: Copy>(
    samples: &[T],
    width: u32,
    height: u32,
    channels: u8,
    band: usize,
    mode: Mode,
    output: &mut [u8],
    convert: impl Fn(T) -> u8,
) -> Result<(), PilError> {
    let count = native_sample_count(width, height, channels)?;
    let samples = samples
        .get(..count)
        .ok_or_else(|| mode_carrier_mismatch(mode))?;
    for (destination, sample) in output
        .iter_mut()
        .zip(samples.iter().skip(band).step_by(usize::from(channels)))
    {
        *destination = convert(*sample);
    }
    Ok(())
}

/// Check the caller's one-byte-per-pixel output contract before writing.
pub(crate) fn validate_band_output(
    width: u32,
    height: u32,
    output: &[u8],
) -> Result<(), PilError> {
    let expected = crate::checked_dims::CheckedDims::new_allow_empty(width, height, 1)?.total_pixels();
    if output.len() != expected {
        return Err(PilError::InternalError(
            "getdata band output length mismatch".into(),
        ));
    }
    Ok(())
}

/// Validate the native four-byte storage used by Pillow's `I` and `F` modes.
pub(crate) fn validate_scalar_storage(
    samples: &[u8],
    width: u32,
    height: u32,
    mode: Mode,
) -> Result<(), PilError> {
    let expected = crate::checked_dims::CheckedDims::new(width, height, 4)?.total_bytes();
    if samples.len() != expected {
        return Err(PilError::InternalError(format!(
            "{} image has invalid scalar storage",
            mode.pillow_name().unwrap_or("unknown")
        )));
    }
    Ok(())
}
