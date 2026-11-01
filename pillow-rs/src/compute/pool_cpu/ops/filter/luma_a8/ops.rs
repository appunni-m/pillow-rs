use super::super::ops::{core_only, core_only_serial, inputs_for};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::ImageLumaA8;

static MAX_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MaxFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 2 },
);
static MIN_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MinFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 2 },
);
static GAUSSIAN_BLUR_ARCHITECTURES: [ImplementationSupport; 5] =
    crate::compute::pool_cpu::ops::filter::helpers::native_byte_blur_architectures(
        OperationId::GaussianBlur,
        MODE,
        2,
    );
static BOX_BLUR_ARCHITECTURES: [ImplementationSupport; 5] =
    crate::compute::pool_cpu::ops::filter::helpers::native_byte_blur_architectures(
        OperationId::BoxBlur,
        MODE,
        2,
    );
static MEDIAN_FILTER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MedianFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteRankWindow { channels: 2 },
);
static RANK_FILTER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::RankFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteRankWindow { channels: 2 },
);

crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_architectures!(MODE, 2);
// NO_PILLOW_SUPPORT: Pillow 12.2 Kernel is documented for L/RGB; this mode extends it.
crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_arch_leaf!(ImageLumaA8, GrayAlphaImage, 2, true);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_architectures!(MODE, 2);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_arch_leaf!(ImageLumaA8, GrayAlphaImage, 2, la);

pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        // NO_PILLOW_SUPPORT: Pillow 12.2 Kernel is documented for L and RGB only;
        // this native mode strategy extends that contract.
        OperationId::Filter3x3 => core_only(OperationId::Filter3x3, MODE, FILTER3X3_ARCHITECTURES),
        // NO_PILLOW_SUPPORT: Pillow 12.2 Kernel is documented for L and RGB only;
        // this native mode strategy extends that contract.
        OperationId::Filter5x5 => core_only(OperationId::Filter5x5, MODE, FILTER5X5_ARCHITECTURES),
        OperationId::GaussianBlur => ModeSupport::cpu(
            OperationId::GaussianBlur,
            MODE,
            crate::compute::registry::support::OutputModeRule::Preserve,
            inputs_for(OperationId::GaussianBlur),
            &GAUSSIAN_BLUR_ARCHITECTURES,
        ),
        OperationId::BoxBlur => ModeSupport::cpu(
            OperationId::BoxBlur,
            MODE,
            OutputModeRule::Preserve,
            inputs_for(OperationId::BoxBlur),
            &BOX_BLUR_ARCHITECTURES,
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

pub(in crate::compute::pool_cpu::ops::filter) fn execute_architecture_blur(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
    context: &crate::compute::ExecutionContext,
) -> Result<DynamicImage, PilError> {
    crate::compute::pool_cpu::ops::filter::helpers::validate_native_byte_blur_selection(
        selection, context,
    )?;
    if let super::super::ops::FilterRequest::GaussianBlur { sigma } = request {
        if super::helpers::gaussian_blur_box_parameters(sigma).0 == 1 {
            return super::helpers::execute_gaussian_blur_radius_one(image, sigma);
        }
    }
    super::helpers::execute_native_byte_blur(
        image,
        MODE,
        2,
        request,
        selection.implementation.architecture,
        context,
    )
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    if matches!(request, super::super::ops::FilterRequest::GaussianBlur { .. }) {
        return execute_gaussian_blur(image, request, selection);
    }
    if selection.implementation.architecture != Architecture::Default
        && matches!(
            request,
            super::super::ops::FilterRequest::MaxFilter { .. }
                | super::super::ops::FilterRequest::MinFilter { .. }
        )
    {
        return crate::compute::pool_cpu::ops::filter::helpers::execute_native_byte_extrema(
            image, request, MODE, 2, selection.implementation.architecture,
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
            image, request, MODE, 2, selection.implementation.architecture,
        );
    }

    if matches!(request, super::super::ops::FilterRequest::Filter5x5 { .. }) {
        if selection.implementation.architecture != Architecture::Default {
            if let Some(output) = execute_filter5x5_small_uniform(image, request)? {
                return Ok(output);
            }
        }
        return match selection.implementation.architecture {
            Architecture::Default => default::execute(image, request),
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            Architecture::Neon => super::neon::execute(image, request),
            Architecture::Sse2
            | Architecture::Avx
            | Architecture::AvxFma
            | Architecture::Avx2
            | Architecture::Avx2Fma
            | Architecture::WasmSimd128 => execute_filter5x5_vectorized(image, request),
            _ => Err(PilError::InternalError(
                "filter selected an unowned Filter5x5 architecture".into(),
            )),
        };
    }
    if matches!(request, super::super::ops::FilterRequest::Filter3x3 { .. }) {
        return match selection.implementation.architecture {
            Architecture::Default => default::execute(image, request),
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            Architecture::Neon => super::neon::execute_filter3x3(image, request),
            Architecture::Sse2
            | Architecture::Avx
            | Architecture::AvxFma
            | Architecture::Avx2
            | Architecture::Avx2Fma
            | Architecture::WasmSimd128 => execute_filter3x3_vectorized(image, request),
            _ => Err(PilError::InternalError(
                "filter selected an unowned Filter3x3 architecture".into(),
            )),
        };
    }
    match selection.implementation.architecture {
        Architecture::Default => default::execute(image, request),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => match request {
            request @ super::super::ops::FilterRequest::Filter3x3 { .. } => {
                super::neon::execute_filter3x3(image, request)
            }
            request @ super::super::ops::FilterRequest::Filter5x5 { .. } => {
                super::neon::execute(image, request)
            }
            _ => Err(PilError::InternalError(
                "NEON filter selection received a non-convolution request".into(),
            )),
        },
        _ => Err(PilError::InternalError("filter selected an unowned architecture".into())),
    }
}

fn execute_gaussian_blur(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let super::super::ops::FilterRequest::GaussianBlur { sigma } = request else {
        return default::execute(image, request);
    };
    match selection.implementation.architecture {
        Architecture::Default => default::execute(image, request),
        Architecture::Avx2
        | Architecture::Avx
        | Architecture::Sse2
        | Architecture::Neon
        | Architecture::WasmSimd128 => {
            super::helpers::execute_gaussian_blur_radius_one(image, sigma)
        }
        _ => Err(PilError::InternalError(
            "filter selected an unowned LA GaussianBlur architecture".into(),
        )),
    }
}
