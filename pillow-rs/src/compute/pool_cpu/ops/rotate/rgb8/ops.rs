//! Exact ImageRgb8 Rotate support and serial strategy.

use crate::compute::pool_cpu::helpers::execution::prepared::Prepared;
use crate::compute::pool_cpu::ops::rotate::helpers::strategy;
use crate::compute::pool_cpu::ops::rotate::ops::RotateRequest;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) type Input = crate::raster::RgbImage;

pub(crate) struct Rotate<'a> {
    input: &'a Input,
    request: RotateRequest,
}

pub(crate) type Strategy<'a> = Prepared<Rotate<'a>>;

static ARCHITECTURES: [ImplementationSupport; 4] = [
    ImplementationSupport::cpu(
        OperationId::Rotate,
        Mode::ImageRgb8,
        Architecture::Avx2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx2"
        )),
        &[Architecture::Avx2],
        StrategyGuard::RotateVectorBytes {
            channels: 3,
            force_nearest: false,
            bilinear: true,
        },
    ),
    ImplementationSupport::cpu(
        OperationId::Rotate,
        Mode::ImageRgb8,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2",
            not(target_feature = "avx2")
        )),
        &[Architecture::Sse2],
        StrategyGuard::RotateVectorBytes {
            channels: 3,
            force_nearest: false,
            bilinear: true,
        },
    ),
    ImplementationSupport::cpu(
        OperationId::Rotate,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::RotateVectorBytes {
            channels: 3,
            force_nearest: false,
            bilinear: true,
        },
    ),
    ImplementationSupport::cpu(
        OperationId::Rotate,
        Mode::ImageRgb8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::RotateVectorBytes {
            channels: 3,
            force_nearest: false,
            bilinear: true,
        },
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Rotate,
    Mode::ImageRgb8,
    OutputModeRule::Preserve,
    super::super::ops::ROTATE_INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn prepare<'a>(
    input: &'a Input,
    request: RotateRequest,
    context: &ExecutionContext,
) -> Result<Strategy<'a>, PilError> {
    let facts = strategy::facts(input, request);
    let selection = strategy::select(SUPPORT, facts, context)?;
    Ok(Prepared::new(Rotate { input, request }, selection))
}

pub(crate) fn execute(plan: Strategy<'_>) -> Result<DynamicImage, PilError> {
    let architecture = plan.selection().implementation.architecture;
    let rotate = plan.implementation;
    let output = match architecture {
        Architecture::Default => super::default::rotate_impl::execute(rotate.input, rotate.request),
        Architecture::Avx2 => super::avx2::rotate_impl::execute(rotate.input, rotate.request),
        Architecture::Sse2 => super::sse2::rotate_impl::execute(rotate.input, rotate.request),
        Architecture::Neon => super::neon::rotate_impl::execute(rotate.input, rotate.request),
        Architecture::WasmSimd128 => {
            super::wasm_simd128::rotate_impl::execute(rotate.input, rotate.request)
        }
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for Rotate mode ImageRgb8"
        ))),
    }?;
    Ok(DynamicImage::ImageRgb8(output))
}

