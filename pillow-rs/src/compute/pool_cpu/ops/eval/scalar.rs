//! Prepared native-sample point operations for Pillow's scalar modes.

use crate::compute::Architecture;
use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationSupport, RejectionKind, StrategySelection,
};
use crate::error::PilError;
use crate::image::Image;
use crate::raster::{DynamicImage, Mode};

fn classify(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::serial(OperationId::PointScalar, mode, &[]),
        mode => ModeSupport::Rejected {
            mode,
            reason: "point operation not supported for this mode",
            kind: RejectionKind::ValueError,
        },
    }
}

/// Exact support contract for native scalar callable point operations.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::PointScalar, &[], classify);

/// A source-mode and architecture decision prepared before scalar mutation.
pub(crate) struct PreparedScalarPoint {
    image: DynamicImage,
    selection: StrategySelection,
}

pub(crate) fn supports(mode: Mode) -> bool {
    matches!(SUPPORT.for_mode(mode), ModeSupport::Supported { .. })
}

pub(crate) fn prepare(
    image: &Image,
    context: &ExecutionContext,
) -> Result<PreparedScalarPoint, PilError> {
    let mode = match image.known_mode_identity() {
        Some(mode) => mode,
        None => image.materialized_shared()?.mode(),
    };
    let selection = SUPPORT.for_mode(mode).select_serial(context)?;
    let image = image.materialize()?;
    if image.mode() != mode {
        return Err(PilError::InternalError(
            "scalar point mode did not match its DynamicImage variant".into(),
        ));
    }
    Ok(PreparedScalarPoint { image, selection })
}

impl PreparedScalarPoint {
    pub(crate) fn selection(&self) -> StrategySelection {
        self.selection
    }
}

pub(crate) fn execute_affine(
    prepared: PreparedScalarPoint,
    scale: f64,
    offset: f64,
) -> Result<Image, PilError> {
    let PreparedScalarPoint {
        mut image,
        selection,
    } = prepared;
    let mode = selection.implementation.mode;
    match selection.implementation.architecture {
        Architecture::Default => apply_affine(&mut image, mode, scale, offset)?,
        architecture => {
            return Err(PilError::InternalError(format!(
                "unexpected {architecture:?} architecture selected for scalar point mode {mode:?}"
            )));
        }
    }
    Image::from_dynamic_with_mode(image, Some(mode))
}

pub(crate) fn execute_callable<F>(
    prepared: PreparedScalarPoint,
    mut callback: F,
) -> Result<Image, PilError>
where
    F: FnMut(f64) -> Result<f64, PilError>,
{
    let PreparedScalarPoint {
        mut image,
        selection,
    } = prepared;
    let mode = selection.implementation.mode;
    match selection.implementation.architecture {
        Architecture::Default => apply_callable(&mut image, mode, &mut callback)?,
        architecture => {
            return Err(PilError::InternalError(format!(
                "unexpected {architecture:?} architecture selected for scalar point mode {mode:?}"
            )));
        }
    }
    Image::from_dynamic_with_mode(image, Some(mode))
}

fn apply_affine(
    image: &mut DynamicImage,
    mode: Mode,
    scale: f64,
    offset: f64,
) -> Result<(), PilError> {
    match (image, mode) {
        (DynamicImage::ImageI32(pixels), Mode::ImageI32) => {
            super::i32::default::point_impl::affine(pixels, scale, offset);
        }
        (DynamicImage::ImageF32(pixels), Mode::ImageF32) => {
            super::f32::default::point_impl::affine(pixels, scale, offset);
        }
        (DynamicImage::ImageI16(pixels), Mode::ImageI16) => {
            super::i16::default::point_impl::affine(pixels, scale, offset)?;
        }
        (DynamicImage::ImageI16L(pixels), Mode::ImageI16L) => {
            super::i16l::default::point_impl::affine(pixels, scale, offset)?;
        }
        (DynamicImage::ImageI16B(pixels), Mode::ImageI16B) => {
            super::i16b::default::point_impl::affine(pixels, scale, offset)?;
        }
        (DynamicImage::ImageI16N(pixels), Mode::ImageI16N) => {
            super::i16n::default::point_impl::affine(pixels, scale, offset)?;
        }
        _ => return Err(mode_mismatch()),
    }
    Ok(())
}

fn apply_callable<F>(image: &mut DynamicImage, mode: Mode, callback: &mut F) -> Result<(), PilError>
where
    F: FnMut(f64) -> Result<f64, PilError>,
{
    match (image, mode) {
        (DynamicImage::ImageI32(pixels), Mode::ImageI32) => {
            super::i32::default::point_impl::callable(pixels, callback)?;
        }
        (DynamicImage::ImageF32(pixels), Mode::ImageF32) => {
            super::f32::default::point_impl::callable(pixels, callback)?;
        }
        (DynamicImage::ImageI16(pixels), Mode::ImageI16) => {
            super::i16::default::point_impl::callable(pixels, callback)?;
        }
        (DynamicImage::ImageI16L(pixels), Mode::ImageI16L) => {
            super::i16l::default::point_impl::callable(pixels, callback)?;
        }
        (DynamicImage::ImageI16B(pixels), Mode::ImageI16B) => {
            super::i16b::default::point_impl::callable(pixels, callback)?;
        }
        (DynamicImage::ImageI16N(pixels), Mode::ImageI16N) => {
            super::i16n::default::point_impl::callable(pixels, callback)?;
        }
        _ => return Err(mode_mismatch()),
    }
    Ok(())
}

fn mode_mismatch() -> PilError {
    PilError::InternalError("scalar point mode did not match its DynamicImage variant".into())
}
