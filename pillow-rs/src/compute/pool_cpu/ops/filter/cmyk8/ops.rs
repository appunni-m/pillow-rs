use super::super::ops::{core_only, core_only_serial, inputs_for};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::ImageCmyk8;

static MAX_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MaxFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 4 },
);
static MIN_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MinFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 4 },
);
static MEDIAN_FILTER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MedianFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteRankWindow { channels: 4 },
);
static RANK_FILTER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::RankFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteRankWindow { channels: 4 },
);

crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_architectures!(MODE, 4);
// NO_PILLOW_SUPPORT: Pillow 12.2 Kernel is documented for L/RGB; this mode extends it.
crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_arch_leaf!(ImageCmyk8, RgbaImage, 4);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_architectures!(MODE, 4);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_arch_leaf!(ImageCmyk8, RgbaImage, 4);


pub(super) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        // NO_PILLOW_SUPPORT: Pillow 12.2 Kernel is documented for L and RGB only;
        // this native mode strategy extends that contract.
        OperationId::Filter3x3 => core_only(OperationId::Filter3x3, MODE, FILTER3X3_ARCHITECTURES),
        // NO_PILLOW_SUPPORT: Pillow 12.2 Kernel is documented for L and RGB only;
        // this native mode strategy extends that contract.
        OperationId::Filter5x5 => core_only(OperationId::Filter5x5, MODE, FILTER5X5_ARCHITECTURES),
        OperationId::GaussianBlur => ModeSupport::serial(
            OperationId::GaussianBlur, Mode::ImageCmyk8, inputs_for(OperationId::GaussianBlur),
        ),
        OperationId::BoxBlur => ModeSupport::serial(
            OperationId::BoxBlur, Mode::ImageCmyk8, inputs_for(OperationId::BoxBlur),
        ),
        // NO_PILLOW_SUPPORT: Pillow 12.2 rank filters require scalar storage;
        // this native mode strategy filters native channels.
        OperationId::MedianFilter => core_only(OperationId::MedianFilter, MODE, &MEDIAN_FILTER_ARCHITECTURES),
        // NO_PILLOW_SUPPORT: Pillow 12.2 rank filters require scalar storage;
        // this native mode strategy filters native channels.
        OperationId::MaxFilter => core_only(OperationId::MaxFilter, MODE, &MAX_FILTER_ARCHITECTURES),
        // NO_PILLOW_SUPPORT: Pillow 12.2 rank filters require scalar storage;
        // this native mode strategy filters native channels.
        OperationId::MinFilter => core_only(OperationId::MinFilter, MODE, &MIN_FILTER_ARCHITECTURES),
        // NO_PILLOW_SUPPORT: Pillow 12.2 rank filters require scalar storage;
        // this native mode strategy filters native channels.
        OperationId::RankFilter => core_only(OperationId::RankFilter, MODE, &RANK_FILTER_ARCHITECTURES),
        _ => ModeSupport::Rejected {
            mode: MODE,
            reason: "non-filter operation reached filter mode strategy",
            kind: crate::compute::registry::support::RejectionKind::InternalError,
        },
    }
}

pub(super) fn execute(
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
            image, request, MODE, 4, selection.implementation.architecture,
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
            image, request, MODE, 4, selection.implementation.architecture,
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
            default::execute,
            execute_filter5x5_vectorized
        );
    }
    if matches!(request, super::super::ops::FilterRequest::Filter3x3 { .. }) {
        return match selection.implementation.architecture {
            Architecture::Default => default::execute(image, request),
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
    default::execute(image, request)
}
