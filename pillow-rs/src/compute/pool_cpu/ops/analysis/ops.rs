//! Direct analysis and GetData support contracts with prepared CPU selection.

use crate::compute::{Backend, ExecutionContext};
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationSupport, OutputModeRule,
    PaletteKnowledge, RejectionKind, RequestFacts, StrategyInputs, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

use super::helpers::get_data::{
    GetDataOutput, GetDataRequest,
};
use super::helpers::projection::{
    ProjectionAxes, project_lab, project_native_bytes, project_stored_words,
};

const NO_INPUTS: &[InputContract] = &[];
const NO_RASTER_OUTPUT: OutputModeRule = OutputModeRule::NoRasterResult;
const GET_DATA_BAND_1_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetDataBandIndex { bands: 1 }];
const GET_DATA_BAND_2_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetDataBandIndex { bands: 2 }];
const GET_DATA_BAND_3_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetDataBandIndex { bands: 3 }];
const GET_DATA_BAND_4_INPUTS: &[InputContract] =
    &[InputContract::CallerValidatedGetDataBandIndex { bands: 4 }];

fn get_data_band_inputs(mode: Mode) -> &'static [InputContract] {
    match get_data_band_count(mode) {
        1 => GET_DATA_BAND_1_INPUTS,
        2 => GET_DATA_BAND_2_INPUTS,
        3 => GET_DATA_BAND_3_INPUTS,
        4 => GET_DATA_BAND_4_INPUTS,
        _ => NO_INPUTS,
    }
}

/// Returns the number of logical bands declared for an exact raster identity.
pub(crate) fn get_data_band_count(mode: Mode) -> usize {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImagePalette8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => 1,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8
        | Mode::ImageLumaA16 => 2,
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8
        | Mode::ImageRgb16 | Mode::ImageRgb32F => 3,
        Mode::ImageRgba8 | Mode::ImageRgbPremultiplied8 | Mode::ImageRgbx8
        | Mode::ImageCmyk8 | Mode::ImageRgba16 | Mode::ImageRgba32F => 4,
    }
}

fn classify_get_data_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::GetData,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
        // NO_PILLOW_SUPPORT: GetData narrows native samples at this byte-read boundary.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetData,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "native high-depth GetData extends the Pillow mode contract",
        ),
    }
}

/// Exhaustive support declaration for the direct full-buffer GetData read.
pub(crate) static GET_DATA_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetData, NO_INPUTS, classify_get_data_mode);

fn classify_get_data_band_mode(mode: Mode) -> ModeSupport {
    let inputs = get_data_band_inputs(mode);
    match mode {
        // NO_PILLOW_SUPPORT: these internal identities expose native samples through GetData.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetDataBand,
            mode,
            NO_RASTER_OUTPUT,
            inputs,
            &[],
            "native high-depth GetDataBand extends the Pillow mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::GetDataBand,
            mode,
            NO_RASTER_OUTPUT,
            inputs,
            &[],
        ),
    }
}

/// Allocated band reads compose caller-buffer and ExtractBand strategies where eligible.
pub(crate) static GET_DATA_BAND_SUPPORT: OperationSupport = OperationSupport::composed(
    OperationId::GetDataBand,
    NO_INPUTS,
    classify_get_data_band_mode,
    &[OperationId::GetDataBandInto, OperationId::ExtractBand],
);

fn classify_get_data_band_into_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageI32
        | Mode::ImageF32 => ModeSupport::Rejected {
            mode,
            reason: "image has wrong mode",
            kind: RejectionKind::ValueError,
        },
        // NO_PILLOW_SUPPORT: internal high-depth bands are native Rust extensions.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetDataBandInto,
            mode,
            NO_RASTER_OUTPUT,
            get_data_band_inputs(mode),
            &[],
            "native high-depth GetDataBandInto extends the Pillow mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8 => ModeSupport::cpu(
            OperationId::GetDataBandInto,
            mode,
            NO_RASTER_OUTPUT,
            get_data_band_inputs(mode),
            &[],
        ),
    }
}

/// Caller-owned band reads declare the existing scalar-mode rejection contract.
pub(crate) static GET_DATA_BAND_INTO_SUPPORT: OperationSupport =
    OperationSupport::composed(
        OperationId::GetDataBandInto,
        NO_INPUTS,
        classify_get_data_band_into_mode,
        &[OperationId::ExtractBand],
    );

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
///
/// GPU eligibility remains here until its support descriptor can compose this
/// CPU operation's terminal output contract.
pub(crate) fn supports_get_data_band_terminal(mode: Mode, backend: Backend) -> bool {
    match backend {
        Backend::Simd => matches!(
            mode,
            Mode::ImageLumaA8
                | Mode::ImagePaletteAlpha8
                | Mode::ImageRgb8
                | Mode::ImageRgba8
                | Mode::ImageRgbPremultiplied8
                | Mode::ImageRgbx8
                | Mode::ImageCmyk8
                | Mode::ImageHsv8
                | Mode::ImageYCbCr8
        ),
        Backend::Gpu => !matches!(
            mode,
            Mode::ImagePalette8 | Mode::ImageLab8 | Mode::ImageI32 | Mode::ImageF32
        ),
        Backend::Cpu => false,
    }
}

/// Selects the direct full-buffer read for the mode carried by the raster.
pub(crate) fn select_get_data(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_SUPPORT.for_mode(mode).select_serial(context)
}

/// Selects the allocated band-read strategy for the exact raster mode.
pub(crate) fn select_get_data_band(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_BAND_SUPPORT
        .for_mode(mode)
        .select_serial(context)
}

/// Selects the caller-owned band-read CPU strategy for the exact raster mode.
pub(crate) fn select_get_data_band_into(
    mode: Mode,
    context: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    GET_DATA_BAND_INTO_SUPPORT
        .for_mode(mode)
        .select_serial(context)
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
        return Err(super::helpers::get_data::mode_carrier_mismatch(
            selection.implementation.mode,
        ));
    }
    match image {
        DynamicImage::One(image) => super::one::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageLuma8(image) => super::luma8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageLumaA8(image) => super::luma_a8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageLaPremultiplied8(image) => super::la_premultiplied8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImagePalette8(image) => super::palette8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImagePaletteAlpha8(image) => super::palette_alpha8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageRgb8(image) => super::rgb8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageRgba8(image) => super::rgba8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageRgbPremultiplied8(image) => super::rgb_premultiplied8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageRgbx8(image) => super::rgbx8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageCmyk8(image) => super::cmyk8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageYCbCr8(image) => super::ycbcr8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageLab8(image) => super::lab8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageHsv8(image) => super::hsv8::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageI32(image) => super::i32::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageF32(image) => super::f32::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageI16(image) => super::i16::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageI16L(image) => super::i16l::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageI16B(image) => super::i16b::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageI16N(image) => super::i16n::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageLumaA16(image) => super::luma_a16::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageRgb16(image) => super::rgb16::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageRgba16(image) => super::rgba16::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageRgb32F(image) => super::rgb32f::ops::execute_get_data(image, selection, request),
        DynamicImage::ImageRgba32F(image) => super::rgba32f::ops::execute_get_data(image, selection, request),
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
    super::helpers::get_data::validate_band_output(image.width(), image.height(), &output)?;
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
    super::helpers::get_data::validate_band_output(image.width(), image.height(), output)?;
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
    super::palette8::ops::write_palette_indices_band(indices, selection, band, output)
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
            super::i16::ops::integer_samples(image, selection)
        }
        (Mode::ImageI16L, DynamicImage::ImageI16L(image)) => {
            super::i16l::ops::integer_samples(image, selection)
        }
        (Mode::ImageI16B, DynamicImage::ImageI16B(image)) => {
            super::i16b::ops::integer_samples(image, selection)
        }
        (Mode::ImageI16N, DynamicImage::ImageI16N(image)) => {
            super::i16n::ops::integer_samples(image, selection)
        }
        (_, _) if matches!(mode, Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N) => {
            Err(super::helpers::get_data::mode_carrier_mismatch(mode))
        }
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
            super::i32::ops::scalar_samples(image, selection)
        }
        (Mode::ImageF32, DynamicImage::ImageF32(image)) => {
            super::f32::ops::scalar_samples(image, selection)
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
        DynamicImage::One(image) => {
            super::one::ops::get_data_view(image, selection).map(Some)
        }
        DynamicImage::ImageLuma8(image) => {
            super::luma8::ops::get_data_view(image, selection).map(Some)
        }
        DynamicImage::ImagePalette8(image) => {
            super::palette8::ops::get_data_view(image, selection).map(Some)
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
    super::palette8::ops::palette_indices_view(indices, selection)
}

/// Result and execution evidence produced by a prepared projection strategy.
#[derive(Debug)]
pub(crate) struct ProjectionOutput {
    /// Horizontal and vertical `0`/`1` projection axes.
    pub(crate) axes: ProjectionAxes,
    /// Number of full vector blocks processed by an architecture kernel.
    pub(crate) vector_blocks: u64,
    /// Number of scalar pixels processed by a vector kernel's tail path.
    pub(crate) scalar_tail: u64,
}

impl ProjectionOutput {
    pub(crate) fn serial(axes: ProjectionAxes) -> Self {
        Self { axes, vector_blocks: 0, scalar_tail: 0 }
    }

    pub(crate) fn vector(axes: ProjectionAxes, vector_blocks: u64, scalar_tail: u64) -> Self {
        Self { axes, vector_blocks, scalar_tail }
    }
}

fn classify_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::GetProjection,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
        ),
        Mode::ImageRgb8 => super::rgb8::ops::SUPPORT,
        Mode::ImageCmyk8 => super::cmyk8::ops::SUPPORT,
        // NO_PILLOW_SUPPORT: GetProjection scans internal high-depth rasters
        // directly using their stored native samples.
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetProjection,
            mode,
            NO_RASTER_OUTPUT,
            NO_INPUTS,
            &[],
            "native high-depth GetProjection extends the Pillow mode contract",
        ),
    }
}

/// Exhaustive support declaration for direct materialized GetProjection.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetProjection, NO_INPUTS, classify_mode);

/// Prepared direct projection, retaining the exact mode and architecture choice.
pub(crate) struct PreparedProjection<'a> {
    image: &'a DynamicImage,
    mode: Mode,
    selection: StrategySelection,
}

impl PreparedProjection<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<ProjectionOutput, PilError> {
        if self.image.mode() != self.mode {
            return Err(PilError::InternalError(
                "GetProjection prepared mode did not match its DynamicImage tag".into(),
            ));
        }
        match self.mode {
            Mode::ImageRgb8 => {
                let DynamicImage::ImageRgb8(image) = self.image else {
                    return Err(storage_error(self.mode));
                };
                super::rgb8::ops::execute(image, self.selection)
            }
            Mode::ImageCmyk8 => {
                let DynamicImage::ImageCmyk8(image) = self.image else {
                    return Err(storage_error(self.mode));
                };
                super::cmyk8::ops::execute(image, self.selection)
            }
            Mode::ImageLab8 => {
                let DynamicImage::ImageLab8(image) = self.image else {
                    return Err(storage_error(self.mode));
                };
                Ok(ProjectionOutput::serial(project_lab(image)))
            }
            Mode::ImageI32 | Mode::ImageF32 => {
                let raw = match self.image {
                    DynamicImage::ImageI32(image) | DynamicImage::ImageF32(image) => {
                        image.as_raw()
                    }
                    _ => return Err(storage_error(self.mode)),
                };
                Ok(ProjectionOutput::serial(project_stored_words(
                    raw,
                    self.image.width(),
                    self.image.height(),
                )?))
            }
            mode => {
                let bytes_per_pixel = native_pixel_bytes(mode);
                Ok(ProjectionOutput::serial(project_native_bytes(
                    self.image.as_bytes(),
                    self.image.width(),
                    self.image.height(),
                    bytes_per_pixel,
                )?))
            }
        }
    }
}

/// Resolve the exact mode declaration and architecture before scanning pixels.
pub(crate) fn prepare<'a>(
    image: &'a DynamicImage,
    context: &ExecutionContext,
) -> Result<PreparedProjection<'a>, PilError> {
    let mode = image.mode();
    let selection = select(context, mode, image.width(), image.height(), image.as_bytes().len())?;
    Ok(PreparedProjection { image, mode, selection })
}

/// Execute the native RGB8 strategy for terminal CPU fusion.
pub(crate) fn execute_rgb8_in_context(
    context: &ExecutionContext,
    image: &crate::raster::RgbImage,
) -> Result<ProjectionOutput, PilError> {
    let selection = select(
        context,
        Mode::ImageRgb8,
        image.width(),
        image.height(),
        image.as_raw().len(),
    )?;
    super::rgb8::ops::execute(image, selection)
}

/// Execute the native CMYK8 strategy for terminal CPU fusion.
pub(crate) fn execute_cmyk8_in_context(
    context: &ExecutionContext,
    image: &crate::raster::RgbaImage,
) -> Result<ProjectionOutput, PilError> {
    let selection = select(
        context,
        Mode::ImageCmyk8,
        image.width(),
        image.height(),
        image.as_raw().len(),
    )?;
    super::cmyk8::ops::execute(image, selection)
}

fn select(
    context: &ExecutionContext,
    mode: Mode,
    width: u32,
    height: u32,
    byte_len: usize,
) -> Result<StrategySelection, PilError> {
    SUPPORT.for_mode(mode).select(
        context,
        StrategyInputs {
            width,
            height,
            byte_len: Some(byte_len),
            palette: PaletteKnowledge::Unknown,
            request: RequestFacts::Unspecified,
        },
    )
}

fn native_pixel_bytes(mode: Mode) -> usize {
    match mode {
        Mode::One | Mode::ImageLuma8 | Mode::ImagePalette8 => 1,
        Mode::ImageLumaA8 | Mode::ImageLaPremultiplied8 | Mode::ImagePaletteAlpha8 => 2,
        Mode::ImageRgb8 | Mode::ImageYCbCr8 | Mode::ImageLab8 | Mode::ImageHsv8 => 3,
        Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageI32
        | Mode::ImageF32 => 4,
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => 2,
        Mode::ImageLumaA16 => 4,
        Mode::ImageRgb16 => 6,
        Mode::ImageRgba16 => 8,
        Mode::ImageRgb32F => 12,
        Mode::ImageRgba32F => 16,
    }
}

fn storage_error(mode: Mode) -> PilError {
    PilError::InternalError(format!(
        "GetProjection mode {mode:?} did not match its exact raster variant"
    ))
}


