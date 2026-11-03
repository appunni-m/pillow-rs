//! Direct GetData orchestration over prepared exact-mode owners.

use crate::compute::{Architecture, Backend, ExecutionContext};
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::GET_DATA_BAND_INTO_SUPPORT;
use super::super::helpers::get_data::{GetDataOutput, GetDataRequest};

/// Returns the band count or the existing mode error for caller-owned reads.
pub(crate) fn get_data_band_into_count(mode: Mode) -> Result<usize, PilError> {
    match GET_DATA_BAND_INTO_SUPPORT.for_mode(mode) {
        ModeSupport::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
        ModeSupport::Supported { inputs, .. } => inputs
            .iter()
            .find_map(|input| match input {
                InputContract::CallerValidatedGetDataBandIndex { bands } => {
                    Some(usize::from(*bands))
                }
                _ => None,
            })
            .ok_or_else(|| PilError::InternalError("GetData band count was not declared".into())),
    }
}

/// Returns whether caller-owned band reads have a declared mode strategy.
pub(crate) fn supports_get_data_band_into(mode: Mode) -> bool {
    matches!(
        GET_DATA_BAND_INTO_SUPPORT.for_mode(mode),
        ModeSupport::Supported { .. }
    )
}

/// Returns whether the existing terminal band route can preserve this mode.
// TODO(pillow-rs#3ptb): Move GPU eligibility into its support declaration when
// that descriptor can compose this CPU terminal contract.
pub(crate) fn supports_get_data_band_terminal(
    mode: Mode,
    backend: Backend,
    context: &ExecutionContext,
) -> bool {
    match backend {
        Backend::Simd => {
            // GetData's SIMD terminal is the declared ExtractBand child route.
            let ModeSupport::Supported { architectures, .. } =
                OperationId::ExtractBand.support().for_mode(mode)
            else {
                return false;
            };
            context.backend() == Backend::Cpu
                && architectures.iter().any(|implementation| {
                    let architecture = implementation.id.architecture;
                    architecture != Architecture::Default
                        && implementation.compiled
                        && context.permits_architecture(architecture)
                        && context.supports_architecture(architecture)
                        && implementation
                            .requires
                            .iter()
                            .all(|&required| context.supports_architecture(required))
                })
        }
        Backend::Gpu => !matches!(
            mode,
            Mode::ImagePalette8
                | Mode::ImageLab8
                | Mode::ImageI32
                | Mode::ImageF32
                | Mode::ImageLumaA16
                | Mode::ImageRgb16
                | Mode::ImageRgba16
                | Mode::ImageRgb32F
                | Mode::ImageRgba32F
        ),
        Backend::Cpu => false,
    }
}


fn validate_get_data_selection(
    selection: StrategySelection,
    mode: Mode,
    operation: OperationId,
) -> Result<(), PilError> {
    let implementation = selection.implementation;
    if implementation.mode != mode || implementation.operation != operation {
        return Err(PilError::InternalError(
            "GetData selection did not match its operation and DynamicImage mode".into(),
        ));
    }
    Ok(())
}

fn execute_get_data_request(
    image: &DynamicImage,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    if image.mode() != selection.implementation.mode {
        return Err(super::super::helpers::get_data::mode_carrier_mismatch(
            selection.implementation.mode,
        ));
    }
    match image {
        DynamicImage::One(image) => super::super::one::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageLuma8(image) => {
            super::super::luma8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageLumaA8(image) => {
            super::super::luma_a8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageLaPremultiplied8(image) => {
            super::super::la_premultiplied8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImagePalette8(image) => {
            super::super::palette8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImagePaletteAlpha8(image) => {
            super::super::palette_alpha8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageRgb8(image) => {
            super::super::rgb8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageRgba8(image) => {
            super::super::rgba8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageRgbPremultiplied8(image) => {
            super::super::rgb_premultiplied8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageRgbx8(image) => {
            super::super::rgbx8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageCmyk8(image) => {
            super::super::cmyk8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageYCbCr8(image) => {
            super::super::ycbcr8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageLab8(image) => {
            super::super::lab8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageHsv8(image) => {
            super::super::hsv8::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageI32(image) => {
            super::super::i32::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageF32(image) => {
            super::super::f32::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageI16(image) => {
            super::super::i16::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageI16L(image) => {
            super::super::i16l::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageI16B(image) => {
            super::super::i16b::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageI16N(image) => {
            super::super::i16n::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageLumaA16(image) => {
            super::super::luma_a16::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageRgb16(image) => {
            super::super::rgb16::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageRgba16(image) => {
            super::super::rgba16::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageRgb32F(image) => {
            super::super::rgb32f::ops::execute_get_data(image, selection, request)
        }
        DynamicImage::ImageRgba32F(image) => {
            super::super::rgba32f::ops::execute_get_data(image, selection, request)
        }
    }
}

/// Run the full-buffer strategy selected for a mode-bearing raster.
pub(crate) fn get_data(
    image: &DynamicImage,
    selection: StrategySelection,
) -> Result<Vec<u8>, PilError> {
    validate_get_data_selection(selection, image.mode(), OperationId::GetData)?;
    match execute_get_data_request(image, selection, GetDataRequest::Full)? {
        GetDataOutput::Bytes(bytes) => Ok(bytes),
        GetDataOutput::BandWritten => Err(PilError::InternalError(
            "GetData full-buffer strategy returned a band result".into(),
        )),
    }
}

/// Allocate and fill one band using the selected GetDataBand strategy.
pub(crate) fn get_data_band(
    image: &DynamicImage,
    band: usize,
    selection: StrategySelection,
) -> Result<Vec<u8>, PilError> {
    validate_get_data_selection(selection, image.mode(), OperationId::GetDataBand)?;
    let dimensions = crate::checked_dims::CheckedDims::new_allow_empty(
        image.width(),
        image.height(),
        1,
    )?;
    let mut output = dimensions.alloc_buffer();
    super::super::helpers::get_data::validate_band_output(image.width(), image.height(), &output)?;
    get_data_band_into_request(image, band, selection, &mut output)?;
    Ok(output)
}

/// Fill one band using the selected caller-owned GetDataBandInto strategy.
pub(crate) fn get_data_band_into(
    image: &DynamicImage,
    band: usize,
    selection: StrategySelection,
    output: &mut [u8],
) -> Result<(), PilError> {
    validate_get_data_selection(selection, image.mode(), OperationId::GetDataBandInto)?;
    super::super::helpers::get_data::validate_band_output(image.width(), image.height(), output)?;
    get_data_band_into_request(image, band, selection, output)
}

fn get_data_band_into_request(
    image: &DynamicImage,
    band: usize,
    selection: StrategySelection,
    output: &mut [u8],
) -> Result<(), PilError> {
    match execute_get_data_request(
        image,
        selection,
        GetDataRequest::Band { index: band, output },
    )? {
        GetDataOutput::BandWritten => Ok(()),
        GetDataOutput::Bytes(_) => Err(PilError::InternalError(
            "GetData band strategy returned full-buffer bytes".into(),
        )),
    }
}

/// Preserve the owned palette-index view while using the Palette8 strategy.
pub(crate) fn get_data_palette_indices_band_into(
    indices: &[u8],
    band: usize,
    selection: StrategySelection,
    output: &mut [u8],
) -> Result<(), PilError> {
    validate_get_data_selection(
        selection,
        Mode::ImagePalette8,
        OperationId::GetDataBandInto,
    )?;
    super::super::palette8::ops::write_palette_indices_band(indices, selection, band, output)
}

/// Return scalar `I;16*` values through the exact mode's GetData implementation.
pub(crate) fn get_data_integer_samples(
    image: &DynamicImage,
    selection: StrategySelection,
) -> Result<Vec<i32>, PilError> {
    let mode = selection.implementation.mode;
    validate_get_data_selection(selection, mode, OperationId::GetData)?;
    match (mode, image) {
        (Mode::ImageI16, DynamicImage::ImageI16(image)) => {
            super::super::i16::ops::integer_samples(image, selection)
        }
        (Mode::ImageI16L, DynamicImage::ImageI16L(image)) => {
            super::super::i16l::ops::integer_samples(image, selection)
        }
        (Mode::ImageI16B, DynamicImage::ImageI16B(image)) => {
            super::super::i16b::ops::integer_samples(image, selection)
        }
        (Mode::ImageI16N, DynamicImage::ImageI16N(image)) => {
            super::super::i16n::ops::integer_samples(image, selection)
        }
        (_, _) if matches!(
            mode,
            Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N
        ) => Err(super::super::helpers::get_data::mode_carrier_mismatch(mode)),
        _ => Err(PilError::InternalError(
            "GetData integer strategy received a non-I;16 mode".into(),
        )),
    }
}

/// Return scalar `I` and `F` values through their exact mode strategies.
pub(crate) fn get_data_scalar_samples(
    image: &DynamicImage,
    selection: StrategySelection,
) -> Result<crate::image::ScalarImageSamples, PilError> {
    let mode = selection.implementation.mode;
    validate_get_data_selection(selection, mode, OperationId::GetData)?;
    match (mode, image) {
        (Mode::ImageI32, DynamicImage::ImageI32(image)) => {
            super::super::i32::ops::scalar_samples(image, selection)
        }
        (Mode::ImageF32, DynamicImage::ImageF32(image)) => {
            super::super::f32::ops::scalar_samples(image, selection)
        }
        (Mode::ImageI32, _) | (Mode::ImageF32, _) => Err(PilError::InternalError(format!(
            "{} image has invalid scalar storage",
            mode.pillow_name().unwrap_or("unknown")
        ))),
        _ => Err(PilError::InternalError(
            "GetData scalar strategy received a non-I/F raster".into(),
        )),
    }
}

/// Borrow an untransformed single-band byte plane for the direct view fast path.
pub(crate) fn get_data_band_byte_view(
    image: &DynamicImage,
    selection: StrategySelection,
) -> Result<Option<&[u8]>, PilError> {
    validate_get_data_selection(selection, image.mode(), OperationId::GetData)?;
    match image {
        DynamicImage::One(image) => super::super::one::ops::get_data_view(image, selection).map(Some),
        DynamicImage::ImageLuma8(image) => {
            super::super::luma8::ops::get_data_view(image, selection).map(Some)
        }
        DynamicImage::ImagePalette8(image) => {
            super::super::palette8::ops::get_data_view(image, selection).map(Some)
        }
        _ => Ok(None),
    }
}

/// Borrow an owned palette index plane under the Palette8 GetData strategy.
pub(crate) fn get_data_palette_indices_view(
    indices: &[u8],
    selection: StrategySelection,
) -> Result<&[u8], PilError> {
    validate_get_data_selection(
        selection,
        Mode::ImagePalette8,
        OperationId::GetData,
    )?;
    super::super::palette8::ops::palette_indices_view(indices, selection)
}
