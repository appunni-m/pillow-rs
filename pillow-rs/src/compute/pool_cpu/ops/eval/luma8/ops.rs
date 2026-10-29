//! Exact ImageLuma8 Eval support and architecture composition.

use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::pool_cpu::ops::eval::helpers::{strategy, vector};
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategyInputs, StrategySelection, SupportAssessment,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, GrayImage};

use super::default::eval_impl as default;

const CHANNELS: usize = 1;
const INPUTS: &[InputContract] = &[
    InputContract::ByteLutEntries { channels: CHANNELS as u8 },
    InputContract::ExactNativeBytes {
        channels: CHANNELS as u8,
        overflow_reason: "Eval native byte dimensions overflowed",
        mismatch_reason: "Eval source byte length did not match its dimensions",
    },
];

static ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::cpu_architectures_for(Mode::ImageLuma8, CHANNELS as u8);

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Eval,
    Mode::ImageLuma8,
    OutputModeRule::Preserve,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(default::Kernel<'a>),
    Sse2(vector::Kernel<'a, CHANNELS>),
    Neon(vector::Kernel<'a, CHANNELS>),
    WasmSimd128(vector::Kernel<'a, CHANNELS>),
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

pub(crate) fn assess(
    input: StrategyInputs,
    ctx: &ExecutionContext,
) -> Result<SupportAssessment, PilError> {
    SUPPORT.assess(ctx, input)
}

pub(crate) fn prepare<'a>(
    input: &'a GrayImage,
    lut: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let facts = strategy::facts(input.width(), input.height(), input.as_raw().len(), lut.len());
    let selection = strategy::select(SUPPORT, facts, ctx)?;
    let implementation = match selection.implementation.architecture {
        Architecture::Default => ArchitectureStrategy::Default(default::prepare(input, lut)),
        Architecture::Sse2 => ArchitectureStrategy::Sse2(vector::prepare::<CHANNELS>(
            input.as_raw(), input.width(), input.height(), lut,
        )?),
        Architecture::Neon => ArchitectureStrategy::Neon(vector::prepare::<CHANNELS>(
            input.as_raw(), input.width(), input.height(), lut,
        )?),
        Architecture::WasmSimd128 => ArchitectureStrategy::WasmSimd128(
            vector::prepare::<CHANNELS>(input.as_raw(), input.width(), input.height(), lut)?,
        ),
        architecture => {
            return Err(PilError::InternalError(format!(
                "unexpected {architecture:?} architecture selected for Eval mode ImageLuma8"
            )));
        }
    };
    Ok(Prepared::new(implementation, selection))
}

fn from_vector(output: vector::Output) -> Result<default::Input, PilError> {
    default::Input::from_raw(output.width, output.height, output.bytes)
        .ok_or_else(|| PilError::InternalError("Eval LUT output dimensions were invalid".into()))
}

pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default(kernel) => {
            default::execute(kernel).map(DynamicImage::ImageLuma8)
        }
        ArchitectureStrategy::Sse2(kernel) => {
            vector::execute_sse2(kernel).and_then(from_vector).map(DynamicImage::ImageLuma8)
        }
        ArchitectureStrategy::Neon(kernel) => {
            vector::execute_neon(kernel).and_then(from_vector).map(DynamicImage::ImageLuma8)
        }
        ArchitectureStrategy::WasmSimd128(kernel) => vector::execute_wasm_simd128(kernel)
            .and_then(from_vector)
            .map(DynamicImage::ImageLuma8),
    }
}
