use super::super::ops::{core_only, core_only_serial, inputs_for, pillow_kernel};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::One;

static MAX_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MaxFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 1 },
);
static MIN_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MinFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 1 },
);
static MEDIAN_FILTER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MedianFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteRankWindow { channels: 1 },
);
static RANK_FILTER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::RankFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteRankWindow { channels: 1 },
);

crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_architectures!(MODE, 1);
crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_arch_leaf!(One, GrayImage, 1);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_architectures!(MODE, 1);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_arch_leaf!(One, GrayImage, 1);


pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        OperationId::Filter3x3 => pillow_kernel(OperationId::Filter3x3, MODE, FILTER3X3_ARCHITECTURES),
        OperationId::Filter5x5 => pillow_kernel(OperationId::Filter5x5, MODE, FILTER5X5_ARCHITECTURES),
        // NO_PILLOW_SUPPORT: Pillow 12.2 blur accepts a narrower uint8 mode set;
        // this native mode strategy extends it.
        OperationId::GaussianBlur => core_only_serial(OperationId::GaussianBlur, MODE),
        // NO_PILLOW_SUPPORT: Pillow 12.2 blur accepts a narrower uint8 mode set;
        // this native mode strategy extends it.
        OperationId::BoxBlur => core_only_serial(OperationId::BoxBlur, MODE),
        OperationId::MedianFilter => ModeSupport::cpu(
            OperationId::MedianFilter, MODE,
            crate::compute::registry::support::OutputModeRule::Preserve,
            inputs_for(OperationId::MedianFilter), &MEDIAN_FILTER_ARCHITECTURES,
        ),
        OperationId::MaxFilter => ModeSupport::cpu(
            OperationId::MaxFilter,
            MODE,
            crate::compute::registry::support::OutputModeRule::Preserve,
            inputs_for(OperationId::MaxFilter),
            &MAX_FILTER_ARCHITECTURES,
        ),
        OperationId::MinFilter => ModeSupport::cpu(
            OperationId::MinFilter,
            MODE,
            crate::compute::registry::support::OutputModeRule::Preserve,
            inputs_for(OperationId::MinFilter),
            &MIN_FILTER_ARCHITECTURES,
        ),
        OperationId::RankFilter => ModeSupport::cpu(
            OperationId::RankFilter, MODE,
            crate::compute::registry::support::OutputModeRule::Preserve,
            inputs_for(OperationId::RankFilter), &RANK_FILTER_ARCHITECTURES,
        ),
        _ => ModeSupport::Rejected {
            mode: MODE,
            reason: "non-filter operation reached filter mode strategy",
            kind: crate::compute::registry::support::RejectionKind::InternalError,
        },
    }
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.architecture != Architecture::Default
        && matches!(
            request,
            super::super::ops::FilterRequest::MaxFilter { .. }
                | super::super::ops::FilterRequest::MinFilter { .. }
        )
    {
        return crate::compute::pool_cpu::ops::filter::helpers::execute_native_byte_extrema(
            image, request, MODE, 1, selection,
        );
    }

    if selection.implementation.architecture != Architecture::Default
        && matches!(
            request,
            super::super::ops::FilterRequest::MedianFilter { .. }
                | super::super::ops::FilterRequest::RankFilter { .. }
        )
    {
        return crate::compute::pool_cpu::ops::filter::helpers::execute_native_byte_rank(
            image, request, MODE, 1, selection,
        );
    }

    if matches!(request, super::super::ops::FilterRequest::Filter5x5 { .. }) {
        if selection.implementation.architecture != Architecture::Default {
            if let Some(output) = execute_filter5x5_small_uniform(image, request)? {
                return Ok(output);
            }
        }
        return crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_arch_dispatch!(
            selection.implementation.architecture,
            image,
            request,
            super::default::execute,
            execute_filter5x5_vectorized
        );
    }
    if matches!(request, super::super::ops::FilterRequest::Filter3x3 { .. }) {
        return match selection.implementation.architecture {
            Architecture::Default => super::default::execute(image, request),
            Architecture::Sse2
            | Architecture::Avx
            | Architecture::AvxFma
            | Architecture::Avx2
            | Architecture::Avx2Fma
            | Architecture::Neon
            | Architecture::WasmSimd128 => execute_filter3x3_vectorized(image, request),
            _ => Err(PilError::InternalError(
                "filter selected an unowned Filter3x3 architecture".into(),
            )),
        };
    }
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError("filter selected an unowned architecture".into()));
    }
    super::default::execute(image, request)
}

pub(in crate::compute::pool_cpu::ops::filter) fn mode_filter_support() -> crate::compute::registry::support::ModeSupport {
    crate::compute::registry::support::ModeSupport::serial(
        crate::compute::registry::support::OperationId::ModeFilter,
        MODE,
        super::super::ops::inputs_for(
            crate::compute::registry::support::OperationId::ModeFilter,
        ),
    )
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute_mode_filter(
    image: &crate::raster::DynamicImage,
    size: u32,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    super::default::execute_mode_filter(image, size)
}
