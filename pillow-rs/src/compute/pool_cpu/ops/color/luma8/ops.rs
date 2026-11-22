//! L-mode support declaration and architecture selection for RemapPalette.

pub(crate) use super::convert::SUPPORT as CONVERT_SUPPORT;
// TODO(pillow-rs#3ptb): record Default/vector architecture selection receipts
// for L-to-P, including fallback when an implementation is unavailable.

use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OperationInputs,
    OutputModeRule, StrategySelection,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

const INPUTS: &[InputContract] = &[InputContract::ExactNativeBytes {
    channels: 1,
    overflow_reason: "palette remap image size overflow",
    mismatch_reason: "palette remap input has wrong native byte length",
}];
const ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::palette_remap_architectures(Mode::ImageLuma8);

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::RemapPalette,
    Mode::ImageLuma8,
    OutputModeRule::Fixed(Mode::ImagePalette8),
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) const CONVERT_TRANSPARENT_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::ConvertTransparent,
    Mode::ImageLuma8,
    OutputModeRule::ByTargetMode,
    super::super::ops::TRANSPARENT_GRAY_BYTE_INPUTS,
    &[],
);

pub(crate) struct Prepared<'a> {
    source: &'a GrayImage,
    dest_map: &'a [u8],
    selection: StrategySelection,
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub(crate) fn execute_transparent(
    source: &GrayImage,
    target: Mode,
    key: [u8; 3],
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_transparent_impl::execute(source, target, key)
        }
        _ => Err(PilError::InternalError(
            "ConvertTransparent selected an undeclared L architecture".into(),
        )),
    }
}

impl Prepared<'_> {
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let bytes = match self.selection.implementation.architecture {
            Architecture::Default => {
                super::default::remap_palette_impl::execute(self.source, self.dest_map)
            }
            Architecture::Sse2 => {
                super::sse2::remap_palette_impl::execute(self.source.as_raw(), self.dest_map)?
            }
            Architecture::Neon => {
                super::neon::remap_palette_impl::execute(self.source.as_raw(), self.dest_map)?
            }
            Architecture::WasmSimd128 => super::wasm_simd128::remap_palette_impl::execute(
                self.source.as_raw(),
                self.dest_map,
            )?,
            _ => {
                return Err(PilError::InternalError(
                    "RemapPalette selected an undeclared L architecture".into(),
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

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    dest_map: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<Prepared<'a>, PilError> {
    let DynamicImage::ImageLuma8(source) = input.image else {
        return Err(PilError::InternalError(
            "L RemapPalette support received another raster mode".into(),
        ));
    };
    let selection = super::super::helpers::strategy::select(SUPPORT, input, ctx)?;
    Ok(Prepared {
        source,
        dest_map,
        selection,
    })
}
