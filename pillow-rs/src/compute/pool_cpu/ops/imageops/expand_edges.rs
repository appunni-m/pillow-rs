//! Prepared nearest-edge expansion for the Pillow `ImagingCore` callback API.
//!
//! All modes retain their typed raster identity. The sample-copy strategy is
//! shared because edge replication has the same semantics for every carrier;
//! mode selection remains explicit in this operation's support declaration.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ModeSupport, OperationId, OperationSupport, PaletteKnowledge, RequestFacts, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

const NO_INPUTS: &[crate::compute::registry::support::InputContract] = &[];

fn pillow_mode_support(mode: Mode) -> ModeSupport {
    ModeSupport::serial(OperationId::ImagingCoreExpand, mode, NO_INPUTS)
}

// NO_PILLOW_SUPPORT: edge replication supports these five internal high-depth
// Mode identities in addition to the Pillow mode set.
fn core_only_mode_support(mode: Mode) -> ModeSupport {
    ModeSupport::cpu_core_only(
        OperationId::ImagingCoreExpand,
        mode,
        crate::compute::registry::support::OutputModeRule::Preserve,
        NO_INPUTS,
        &[],
        "typed native edge replication extends Pillow's ImagingCore.expand modes",
    )
}

pub(super) fn classify_mode(mode: Mode) -> ModeSupport {
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
        | Mode::ImageI16N => pillow_mode_support(mode),
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => core_only_mode_support(mode),
    }
}

pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::ImagingCoreExpand, NO_INPUTS, classify_mode);

pub(crate) struct PreparedExpandEdges<'a> {
    input: &'a DynamicImage,
    xmargin: u32,
    ymargin: u32,
    selection: StrategySelection,
}

impl PreparedExpandEdges<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        validate_selection(&self.selection, self.input.mode())?;
        execute_mode(self.input, self.xmargin, self.ymargin)
    }
}

pub(crate) fn prepare<'a>(
    input: &'a DynamicImage,
    xmargin: u32,
    ymargin: u32,
    context: &ExecutionContext,
) -> Result<PreparedExpandEdges<'a>, PilError> {
    let selection = super::helpers::strategy::select_mode(
        SUPPORT.for_mode(input.mode()),
        input,
        RequestFacts::Unspecified,
        PaletteKnowledge::Unknown,
        context,
    )?;
    Ok(PreparedExpandEdges {
        input,
        xmargin,
        ymargin,
        selection,
    })
}

fn validate_selection(selection: &StrategySelection, mode: Mode) -> Result<(), PilError> {
    let implementation = selection.implementation;
    if implementation.operation == OperationId::ImagingCoreExpand
        && implementation.mode == mode
        && implementation.backend == crate::compute::Backend::Cpu
        && implementation.architecture == crate::compute::Architecture::Default
    {
        Ok(())
    } else {
        Err(PilError::InternalError(
            "ImagingCore expand received a mismatched CPU strategy".into(),
        ))
    }
}

macro_rules! execute_modes {
    ($input:expr, $xmargin:expr, $ymargin:expr; $($mode:ident),+ $(,)?) => {
        match $input {
            $(DynamicImage::$mode(image) => Ok(DynamicImage::$mode(
                super::helpers::expand::expand_edges_buffer(image, $xmargin, $ymargin)?,
            )),)+
        }
    };
}

fn execute_mode(
    input: &DynamicImage,
    xmargin: u32,
    ymargin: u32,
) -> Result<DynamicImage, PilError> {
    execute_modes!(input, xmargin, ymargin;
        One,
        ImageLuma8,
        ImageLumaA8,
        ImageLaPremultiplied8,
        ImagePalette8,
        ImagePaletteAlpha8,
        ImageRgb8,
        ImageRgba8,
        ImageRgbPremultiplied8,
        ImageRgbx8,
        ImageCmyk8,
        ImageYCbCr8,
        ImageLab8,
        ImageHsv8,
        ImageI32,
        ImageF32,
        ImageI16,
        ImageI16L,
        ImageI16B,
        ImageI16N,
        ImageLumaA16,
        ImageRgb16,
        ImageRgba16,
        ImageRgb32F,
        ImageRgba32F,
    )
}
