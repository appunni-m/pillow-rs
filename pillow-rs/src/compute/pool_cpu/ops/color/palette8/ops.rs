//! P-mode support declaration and architecture selection for RemapPalette.

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OperationInputs,
    OutputModeRule, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

const INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 1,
    overflow_reason: "palette remap image size overflow",
    mismatch_reason: "palette remap input has wrong native byte length",
}];
const ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::palette_remap_architectures(Mode::ImagePalette8);

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::RemapPalette,
    Mode::ImagePalette8,
    OutputModeRule::Fixed(Mode::ImagePalette8),
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) struct Prepared<'a> {
    source: &'a GrayImage,
    dest_map: &'a [u8],
    selection: StrategySelection,
}

impl Prepared<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let bytes = match self.selection.implementation.architecture {
            Architecture::Default => {
                super::super::helpers::remap_palette::scalar(self.source.as_raw(), self.dest_map)
            }
            Architecture::Sse2 => super::sse2::remap_palette_impl::execute(
                self.source.as_raw(),
                self.dest_map,
            )?,
            Architecture::Neon => super::neon::remap_palette_impl::execute(
                self.source.as_raw(),
                self.dest_map,
            )?,
            Architecture::WasmSimd128 => super::wasm_simd128::remap_palette_impl::execute(
                self.source.as_raw(),
                self.dest_map,
            )?,
            _ => {
                return Err(PilError::InternalError(
                    "RemapPalette selected an undeclared P architecture".into(),
                ));
            }
        };
        super::super::helpers::remap_palette::palette_image(
            self.source.width(),
            self.source.height(),
            bytes,
        )
    }
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    dest_map: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<Prepared<'a>, PilError> {
    let DynamicImage::ImagePalette8(source) = input.image else {
        return Err(PilError::InternalError(
            "P RemapPalette support received another raster mode".into(),
        ));
    };
    let selection = super::super::helpers::strategy::select(SUPPORT, input, ctx)?;
    Ok(Prepared { source, dest_map, selection })
}
