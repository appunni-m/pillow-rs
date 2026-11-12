//! Exact I32 Point-to-L support and Default architecture composition.

use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, PaletteKnowledge, RequestFacts,
    StrategyInputs, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

const LUT_ENTRIES: usize = 65_536;
const INPUTS: &[InputContract] = &[
    InputContract::PointI32LutEntries {
        entries: LUT_ENTRIES,
    },
    InputContract::ExactNativeBytes {
        channels: 4,
        overflow_reason: "Point I32 native byte dimensions overflowed",
        mismatch_reason: "Point I32 source byte length did not match its dimensions",
    },
];

/// Pillow's I-to-L strategy uses the signed I sample clamped into a 65,536-entry table.
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::PointModeOutput,
    Mode::ImageI32,
    OutputModeRule::Fixed(Mode::ImageLuma8),
    INPUTS,
    &[],
);

/// A materialized I raster and its selected Default point-to-L strategy.
pub(crate) struct Strategy<'a> {
    input: &'a RgbaImage,
    lut: &'a [u8],
    selection: StrategySelection,
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: &'a RgbaImage,
    lut: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let facts = StrategyInputs {
        width: input.width(),
        height: input.height(),
        byte_len: Some(input.as_raw().len()),
        palette: PaletteKnowledge::Absent,
        request: RequestFacts::Unspecified,
    }
    .with_eval_lut_entries(lut.len());
    let selection = SUPPORT.select(ctx, facts)?;
    Ok(Strategy {
        input,
        lut,
        selection,
    })
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute(
    strategy: Strategy<'_>,
) -> Result<(DynamicImage, StrategySelection), PilError> {
    let Strategy {
        input,
        lut,
        selection,
    } = strategy;
    let output = match selection.implementation.architecture {
        crate::compute::Architecture::Default => {
            super::default::point_impl::to_luma(input, lut)?
        }
        architecture => {
            return Err(PilError::InternalError(format!(
                "unexpected {architecture:?} architecture selected for PointModeOutput mode I"
            )));
        }
    };
    Ok((DynamicImage::ImageLuma8(output), selection))
}
