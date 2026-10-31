use crate::compute::Architecture;
use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{Mode, RgbaImage};

use super::super::ops::ProjectionOutput;

const SSE2_REQUIRES: &[Architecture] = &[Architecture::Sse2];
const NEON_REQUIRES: &[Architecture] = &[Architecture::Neon];
const WASM_REQUIRES: &[Architecture] = &[Architecture::WasmSimd128];
const ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::GetProjection,
        Mode::ImageCmyk8,
        Architecture::Sse2,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        SSE2_REQUIRES,
        StrategyGuard::Always,
    ),
    ImplementationSupport::cpu(
        OperationId::GetProjection,
        Mode::ImageCmyk8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        NEON_REQUIRES,
        StrategyGuard::Always,
    ),
    ImplementationSupport::cpu(
        OperationId::GetProjection,
        Mode::ImageCmyk8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        WASM_REQUIRES,
        StrategyGuard::Always,
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::GetProjection,
    Mode::ImageCmyk8,
    OutputModeRule::NoRasterResult,
    &[],
    ARCHITECTURES,
);

pub(crate) fn execute(
    image: &RgbaImage,
    selection: StrategySelection,
) -> Result<ProjectionOutput, PilError> {
    let result = match selection.implementation.architecture {
        Architecture::Default => {
            return Ok(ProjectionOutput::serial(super::default::projection_impl::execute(image)));
        }
        Architecture::Sse2 => super::sse2::projection_impl::execute(image),
        Architecture::Neon => super::neon::projection_impl::execute(image),
        Architecture::WasmSimd128 => super::wasm_simd128::projection_impl::execute(image),
        architecture => {
            return Err(PilError::InternalError(format!(
                "GetProjection has no CMYK8 {architecture:?} strategy"
            )));
        }
    };
    Ok(ProjectionOutput::vector(result.axes, result.vector_blocks, result.scalar_tail))
}

/// Build CMYK axes from bounded positive writes after selecting the declared
/// mode/architecture strategy. `None` asks the caller to use the full scan.
pub(crate) fn execute_positive_writes(
    context: &ExecutionContext,
    width: u32,
    height: u32,
    ops: &[PipelineOp],
) -> Result<Option<ProjectionOutput>, PilError> {
    super::helpers::positive_writes::execute_in_context(context, width, height, ops)
}
