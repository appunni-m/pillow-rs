use super::super::ops::{core_only, core_only_serial, inputs_for, pillow_kernel};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::ImageLaPremultiplied8;

static MAX_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MaxFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 2 },
);
static MIN_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MinFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 2 },
);

crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_architectures!(MODE, 2);
crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_arch_leaf!(ImageLaPremultiplied8, GrayAlphaImage, 2);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_architectures!(MODE, 2);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_arch_leaf!(ImageLaPremultiplied8, GrayAlphaImage, 2);


pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        OperationId::Filter3x3 => pillow_kernel(OperationId::Filter3x3, MODE, FILTER3X3_ARCHITECTURES),
        OperationId::Filter5x5 => pillow_kernel(OperationId::Filter5x5, MODE, FILTER5X5_ARCHITECTURES),
        OperationId::GaussianBlur => ModeSupport::serial(
            OperationId::GaussianBlur, Mode::ImageLaPremultiplied8, inputs_for(OperationId::GaussianBlur),
        ),
        OperationId::BoxBlur => ModeSupport::serial(
            OperationId::BoxBlur, Mode::ImageLaPremultiplied8, inputs_for(OperationId::BoxBlur),
        ),
        OperationId::BoxBlurXY => ModeSupport::serial(
            OperationId::BoxBlurXY,
            Mode::ImageLaPremultiplied8,
            inputs_for(OperationId::BoxBlurXY),
        ),
        // NO_PILLOW_SUPPORT: Pillow 12.2 rank filters require scalar storage;
        // this native mode strategy filters native channels.
        OperationId::MedianFilter => core_only_serial(OperationId::MedianFilter, MODE),
        // NO_PILLOW_SUPPORT: Pillow 12.2 rank filters require scalar storage;
        // this native mode strategy filters native channels.
        OperationId::MaxFilter => core_only(OperationId::MaxFilter, MODE, &MAX_FILTER_ARCHITECTURES),
        // NO_PILLOW_SUPPORT: Pillow 12.2 rank filters require scalar storage;
        // this native mode strategy filters native channels.
        OperationId::MinFilter => core_only(OperationId::MinFilter, MODE, &MIN_FILTER_ARCHITECTURES),
        // NO_PILLOW_SUPPORT: Pillow 12.2 rank filters require scalar storage;
        // this native mode strategy filters native channels.
        OperationId::RankFilter => core_only_serial(OperationId::RankFilter, MODE),
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
            image, request, MODE, 2, selection,
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
    super::super::ops::serial_mode_filter(MODE)
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute_mode_filter(
    image: &crate::raster::DynamicImage,
    size: u32,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    super::default::execute_mode_filter(image, size)
}
