//! Exact ImagePalette8 Eval support and architecture composition.

use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::pool_cpu::ops::eval::helpers::{strategy, vector};
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategySelection,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayImage, Mode};

const CHANNELS: usize = 1;
const INPUTS: &[InputContract] = &[
    InputContract::ByteLutEntries {
        channels: CHANNELS as u8,
    },
    InputContract::ExactNativeBytes {
        channels: CHANNELS as u8,
        overflow_reason: "Eval native byte dimensions overflowed",
        mismatch_reason: "Eval source byte length did not match its dimensions",
    },
];

static ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::cpu_architectures_for(Mode::ImagePalette8, CHANNELS as u8);
static POINT_ONE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::strategy::cpu_architectures_for_operation(
        OperationId::PointModeOutput,
        Mode::ImagePalette8,
        CHANNELS as u8,
    );

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Eval,
    Mode::ImagePalette8,
    OutputModeRule::Preserve,
    INPUTS,
    &ARCHITECTURES,
);

/// Pillow's P-to-1 point route, using the source mode's selected byte kernel.
pub(crate) const POINT_ONE_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::PointModeOutput,
    Mode::ImagePalette8,
    OutputModeRule::Fixed(Mode::One),
    INPUTS,
    &POINT_ONE_ARCHITECTURES,
);

pub(crate) const FLOAT_LUT_SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::PointFloat,
    Mode::ImagePalette8,
    OutputModeRule::Fixed(Mode::ImageF32),
    super::super::helpers::strategy::FLOAT_LUT_INPUTS,
    &[],
);

pub(crate) enum ArchitectureStrategy<'a> {
    Default(super::default::Kernel<'a>),
    Sse2(vector::Kernel<'a, CHANNELS>),
    Neon(vector::Kernel<'a, CHANNELS>),
    WasmSimd128(vector::Kernel<'a, CHANNELS>),
}

pub(crate) type Strategy<'a> = Prepared<ArchitectureStrategy<'a>>;

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare<'a>(
    input: &'a GrayImage,
    lut: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    prepare_with_support(input, lut, ctx, SUPPORT)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn prepare_point_one<'a>(
    input: &'a GrayImage,
    lut: &'a [u8],
    ctx: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    prepare_with_support(input, lut, ctx, POINT_ONE_SUPPORT)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn prepare_with_support<'a>(
    input: &'a GrayImage,
    lut: &'a [u8],
    ctx: &ExecutionContext,
    support: ModeSupport,
) -> Result<Strategy<'a>, PilError> {
    let facts = strategy::facts(
        input.width(),
        input.height(),
        input.as_raw().len(),
        lut.len(),
    );
    let selection = strategy::select(support, facts, ctx)?;
    let implementation = match selection.implementation.architecture {
        Architecture::Default => ArchitectureStrategy::Default(super::default::prepare(input, lut)),
        Architecture::Sse2 => ArchitectureStrategy::Sse2(vector::prepare::<CHANNELS>(
            input.as_raw(),
            input.width(),
            input.height(),
            lut,
        )?),
        Architecture::Neon => ArchitectureStrategy::Neon(vector::prepare::<CHANNELS>(
            input.as_raw(),
            input.width(),
            input.height(),
            lut,
        )?),
        Architecture::WasmSimd128 => ArchitectureStrategy::WasmSimd128(
            vector::prepare::<CHANNELS>(input.as_raw(), input.width(), input.height(), lut)?,
        ),
        architecture => {
            return Err(PilError::InternalError(format!(
                "unexpected {architecture:?} architecture selected for {:?} mode ImagePalette8",
                selection.implementation.operation
            )));
        }
    };
    Ok(Prepared::new(implementation, selection))
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn from_vector(output: vector::Output) -> Result<super::default::Input, PilError> {
    super::default::Input::from_raw(output.width, output.height, output.bytes)
        .ok_or_else(|| PilError::InternalError("Eval LUT output dimensions were invalid".into()))
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    execute_gray(strategy).map(DynamicImage::ImagePalette8)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute_point_one(strategy: Strategy<'_>) -> Result<DynamicImage, PilError> {
    execute_gray(strategy).map(DynamicImage::One)
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn execute_gray(strategy: Strategy<'_>) -> Result<GrayImage, PilError> {
    match strategy.implementation {
        ArchitectureStrategy::Default(kernel) => super::default::execute(kernel),
        ArchitectureStrategy::Sse2(kernel) => vector::execute_sse2(kernel).and_then(from_vector),
        ArchitectureStrategy::Neon(kernel) => vector::execute_neon(kernel).and_then(from_vector),
        ArchitectureStrategy::WasmSimd128(kernel) => {
            vector::execute_wasm_simd128(kernel).and_then(from_vector)
        }
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute_float_lut(
    input: &GrayImage,
    lut: &[f64],
    ctx: &ExecutionContext,
) -> Result<(DynamicImage, StrategySelection), PilError> {
    let facts = strategy::point_float_facts(
        input.width(),
        input.height(),
        input.as_raw().len(),
        lut.len(),
    );
    let selection = strategy::select(FLOAT_LUT_SUPPORT, facts, ctx)?;
    let output = match selection.implementation.architecture {
        Architecture::Default => {
            super::default::eval_float_impl::execute(input, lut).map(DynamicImage::ImageF32)?
        }
        architecture => {
            return Err(PilError::InternalError(format!(
                "unexpected {architecture:?} architecture selected for PointFloat mode P"
            )));
        }
    };
    Ok((output, selection))
}
