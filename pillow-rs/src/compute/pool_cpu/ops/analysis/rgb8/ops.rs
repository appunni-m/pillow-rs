use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{Mode, RgbImage};

use super::super::ops::ProjectionOutput;
use super::super::helpers::get_data::{
    self as get_data_helpers, GetDataOutput, GetDataRequest,
};
use super::default::get_data_impl;

const SSE2_REQUIRES: &[Architecture] = &[Architecture::Sse2];
const NEON_REQUIRES: &[Architecture] = &[Architecture::Neon];
const WASM_REQUIRES: &[Architecture] = &[Architecture::WasmSimd128];
const ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::GetProjection,
        Mode::ImageRgb8,
        Architecture::Sse2,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        SSE2_REQUIRES,
        StrategyGuard::Always,
    ),
    ImplementationSupport::cpu(
        OperationId::GetProjection,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        NEON_REQUIRES,
        StrategyGuard::Always,
    ),
    ImplementationSupport::cpu(
        OperationId::GetProjection,
        Mode::ImageRgb8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        WASM_REQUIRES,
        StrategyGuard::Always,
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::GetProjection,
    Mode::ImageRgb8,
    OutputModeRule::NoRasterResult,
    &[],
    ARCHITECTURES,
);

pub(crate) fn execute_get_data(
    image: &RgbImage,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture =
        get_data_helpers::selected_architecture(selection, Mode::ImageRgb8, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => {
                Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?))
            }
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(get_data_helpers::missing_architecture(
            Mode::ImageRgb8,
            architecture,
        )),
    }
}

pub(crate) fn execute(
    image: &RgbImage,
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
                "GetProjection has no RGB8 {architecture:?} strategy"
            )));
        }
    };
    Ok(ProjectionOutput::vector(result.axes, result.vector_blocks, result.scalar_tail))
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    rgb8,
    ImageRgb8,
    crate::raster::RgbImage,
);

crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageRgb8,
    crate::raster::RgbImage,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::bytes,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageRgb8,
    crate::raster::RgbImage,
    crate::compute::pool_cpu::ops::analysis::rgb8::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageRgb8,
    crate::raster::RgbImage,
    crate::compute::pool_cpu::ops::analysis::rgb8::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageRgb8,
    crate::raster::RgbImage,
    crate::compute::pool_cpu::ops::analysis::rgb8::default::stat_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::
    declare_getextrema_bytes_owner!(
    ImageRgb8,
    crate::raster::RgbImage,
    crate::compute::pool_cpu::ops::analysis::rgb8::default::extrema_bytes_impl::execute,
);
