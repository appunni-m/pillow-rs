use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::{Mode, RgbaImage};

use super::super::helpers::get_data::{self as get_data_helpers, GetDataOutput, GetDataRequest};
use super::super::ops::ProjectionOutput;
use super::default::get_data_impl;

const SSE2_REQUIRES: &[Architecture] = &[Architecture::Sse2];
const NEON_REQUIRES: &[Architecture] = &[Architecture::Neon];
const WASM_REQUIRES: &[Architecture] = &[Architecture::WasmSimd128];
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::GetProjection,
    Mode::ImageCmyk8,
    OutputModeRule::NoRasterResult,
    &[],
    ARCHITECTURES,
);

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn execute_get_data(
    image: &RgbaImage,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture =
        get_data_helpers::selected_architecture(selection, Mode::ImageCmyk8, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(get_data_helpers::missing_architecture(
            Mode::ImageCmyk8,
            architecture,
        )),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &RgbaImage,
    selection: StrategySelection,
) -> Result<ProjectionOutput, PilError> {
    let result = match selection.implementation.architecture {
        Architecture::Default => {
            return Ok(ProjectionOutput::serial(
                super::default::projection_impl::execute(image),
            ));
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
    Ok(ProjectionOutput::vector(
        result.axes,
        result.vector_blocks,
        result.scalar_tail,
    ))
}

/// Build CMYK axes from bounded positive writes under the prepared strategy.
/// `None` asks the caller to use the full scan.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_positive_writes(
    selection: StrategySelection,
    width: u32,
    height: u32,
    ops: &[PipelineOp],
) -> Option<ProjectionOutput> {
    super::helpers::positive_writes::execute_selected(selection, width, height, ops)
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    cmyk8,
    ImageCmyk8,
    crate::raster::RgbaImage,
    Pillow,
);

crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageCmyk8,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::bytes,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageCmyk8,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::cmyk8::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageCmyk8,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::cmyk8::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageCmyk8,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::cmyk8::default::stat_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_owner!(
    ImageCmyk8,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::cmyk8::default::extrema_bytes_impl::execute,
);

pub(crate) const GET_PIXEL_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::pillow_get_pixel_support(
        crate::raster::Mode::ImageCmyk8,
        crate::compute::registry::support::PixelValueShape::U8Components { bands: 4 },
    );

pub(crate) const GET_PIXEL_RGBA_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::rgba_get_pixel_support(
        crate::raster::Mode::ImageCmyk8,
    );
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_pixel(
    image: &crate::raster::RgbaImage,
    x: u32,
    y: u32,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<crate::image::FormattedPixelValue, crate::error::PilError> {
    super::super::helpers::pixel_access::validate_default_selection(
        selection,
        crate::compute::registry::support::OperationId::GetPixel,
        crate::raster::Mode::ImageCmyk8,
    )?;
    super::default::get_pixel_impl::execute(image, x, y)
}
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::declare_getcolors_support!(
    ImageCmyk8, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_support!(
    ImageCmyk8, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::declare_getextrema_support!(
    ImageCmyk8, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_support!(
    ImageCmyk8, 4, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_support!(
    ImageCmyk8, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::split_support::declare_split_support!(
    ImageCmyk8, pillow, Bands,
);
crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::declare_get_channel_route!(
    ImageCmyk8, 4, Pillow, Child, Luma8,
);

crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::declare_get_data_support!(
    ImageCmyk8, 4, Pillow, Pillow, Pillow,
);
