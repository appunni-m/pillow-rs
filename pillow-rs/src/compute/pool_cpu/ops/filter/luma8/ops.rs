use super::super::ops::inputs_for;
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::ImageLuma8;

static MAX_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = super::super::helpers::native_byte_filter_architectures(
    OperationId::MaxFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 1 },
);
static MIN_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = super::super::helpers::native_byte_filter_architectures(
    OperationId::MinFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 1 },
);
static GAUSSIAN_BLUR_ARCHITECTURES: [ImplementationSupport; 5] =
    super::super::helpers::native_byte_blur_architectures(
        OperationId::GaussianBlur,
        MODE,
        1,
    );
static BOX_BLUR_ARCHITECTURES: [ImplementationSupport; 5] =
    super::super::helpers::native_byte_blur_architectures(
        OperationId::BoxBlur,
        MODE,
        1,
    );
static BOX_BLUR_XY_ARCHITECTURES: [ImplementationSupport; 5] =
    super::super::helpers::native_byte_blur_architectures(
        OperationId::BoxBlurXY,
        MODE,
        1,
    );

super::super::helpers::native_filter3x3_architectures!(MODE, 1);
super::super::helpers::native_filter3x3_arch_leaf!(ImageLuma8, GrayImage, 1);
super::super::helpers::native_filter5x5_architectures!(MODE, 1);
super::super::helpers::native_filter5x5_arch_leaf!(ImageLuma8, GrayImage, 1, luma);

macro_rules! rank_median_architectures {
    ($name:ident, $operation:expr) => {
        const $name: &[ImplementationSupport] = &[
            ImplementationSupport::cpu(
                $operation,
                MODE,
                Architecture::Avx2,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "avx2"
                )),
                &[Architecture::Avx2],
                StrategyGuard::FilterLumaRankWindow,
            ),
            ImplementationSupport::cpu(
                $operation,
                MODE,
                Architecture::Avx,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "avx",
                    not(target_feature = "avx2")
                )),
                &[Architecture::Avx],
                StrategyGuard::FilterLumaRankWindow,
            ),
            ImplementationSupport::cpu(
                $operation,
                MODE,
                Architecture::Sse2,
                cfg!(all(
                    any(target_arch = "x86", target_arch = "x86_64"),
                    target_feature = "sse2",
                    not(target_feature = "avx")
                )),
                &[Architecture::Sse2],
                StrategyGuard::FilterLumaRankWindow,
            ),
            ImplementationSupport::cpu(
                $operation,
                MODE,
                Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                &[Architecture::Neon],
                StrategyGuard::FilterLumaRankWindow,
            ),
            ImplementationSupport::cpu(
                $operation,
                MODE,
                Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                &[Architecture::WasmSimd128],
                StrategyGuard::FilterLumaRankWindow,
            ),
        ];
    };
}

rank_median_architectures!(MEDIAN_FILTER_ARCHITECTURES, OperationId::MedianFilter);
rank_median_architectures!(RANK_FILTER_ARCHITECTURES, OperationId::RankFilter);

pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        OperationId::Filter3x3 => ModeSupport::cpu(
            OperationId::Filter3x3,
            MODE,
            crate::compute::registry::support::OutputModeRule::Preserve,
            inputs_for(OperationId::Filter3x3),
            FILTER3X3_ARCHITECTURES,
        ),
        OperationId::Filter5x5 => ModeSupport::cpu(
            OperationId::Filter5x5,
            MODE,
            OutputModeRule::Preserve,
            inputs_for(OperationId::Filter5x5),
            FILTER5X5_ARCHITECTURES,
        ),
        OperationId::GaussianBlur => ModeSupport::cpu(
            OperationId::GaussianBlur,
            MODE,
            OutputModeRule::Preserve,
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
        OperationId::BoxBlurXY => ModeSupport::cpu(
            OperationId::BoxBlurXY,
            MODE,
            OutputModeRule::Preserve,
            inputs_for(OperationId::BoxBlurXY),
            &BOX_BLUR_XY_ARCHITECTURES,
        ),
        OperationId::MedianFilter => ModeSupport::cpu(
            OperationId::MedianFilter,
            MODE,
            OutputModeRule::Preserve,
            inputs_for(OperationId::MedianFilter),
            MEDIAN_FILTER_ARCHITECTURES,
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
            OperationId::RankFilter,
            MODE,
            OutputModeRule::Preserve,
            inputs_for(OperationId::RankFilter),
            RANK_FILTER_ARCHITECTURES,
        ),
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
    super::super::helpers::validate_native_byte_blur_selection(
        selection, MODE, request, context,
    )?;
    match request {
        super::super::ops::FilterRequest::BoxBlur { radius: 1 } => {
            super::helpers::execute_box_blur_radius_one(image)
        }
        super::super::ops::FilterRequest::BoxBlurXY { radius_x, radius_y, passes }
            if radius_x == 1.0 && radius_y == 1.0 && passes == 1 =>
        {
            super::helpers::execute_box_blur_radius_one(image)
        }
        super::super::ops::FilterRequest::GaussianBlur { sigma }
            if super::super::helpers::gaussian_blur_box_parameters(sigma).0 == 1 =>
        {
            super::helpers::execute_gaussian_blur_radius_one(image, sigma)
        }
        _ => super::super::helpers::execute_native_byte_blur(
            image,
            MODE,
            1,
            request,
            selection,
            context,
        ),
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
        return super::super::helpers::execute_native_byte_extrema(
            image, request, MODE, 1, selection,
        );
    }
    if matches!(
        request,
        super::super::ops::FilterRequest::MedianFilter { .. }
            | super::super::ops::FilterRequest::RankFilter { .. }
    ) {
        return execute_rank(image, request, selection);
    }
    if matches!(request, super::super::ops::FilterRequest::GaussianBlur { .. }) {
        return execute_gaussian_blur(image, request, selection);
    }
    if matches!(
        request,
        super::super::ops::FilterRequest::BoxBlur { .. }
            | super::super::ops::FilterRequest::BoxBlurXY { .. }
    ) {
        return execute_box_blur(image, request, selection);
    }
    if matches!(request, super::super::ops::FilterRequest::Filter5x5 { .. }) {
        return execute_filter5x5(image, request, selection);
    }
    if matches!(request, super::super::ops::FilterRequest::Filter3x3 { .. }) {
        return execute_filter3x3(image, request, selection);
    }
    if selection.implementation.architecture != Architecture::Default {
        return Err(PilError::InternalError("filter selected an unowned architecture".into()));
    }
    super::default::execute(image, request)
}

fn execute_rank(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let (size, rank) = match request {
        super::super::ops::FilterRequest::MedianFilter { size } => {
            (size, size.saturating_mul(size) / 2)
        }
        super::super::ops::FilterRequest::RankFilter { size, rank } => (size, rank),
        _ => return super::default::execute(image, request),
    };
    match selection.implementation.architecture {
        Architecture::Default => super::default::execute(image, request),
        Architecture::Avx2
        | Architecture::Avx
        | Architecture::Sse2
        | Architecture::Neon
        | Architecture::WasmSimd128 => {
            let DynamicImage::ImageLuma8(input) = image else {
                return Err(PilError::InternalError(
                    "L rank strategy received a mismatched raster variant".into(),
                ));
            };
            let result = if size == 3 && matches!(rank, 1 | 4) {
                super::helpers::execute_rank3x3(input, rank)
            } else {
                super::helpers::execute_rank_window(input, size, rank)
            }?;
            Ok(DynamicImage::ImageLuma8(result))
        }
        _ => Err(PilError::InternalError(
            "filter selected an unowned L rank architecture".into(),
        )),
    }
}

fn execute_box_blur(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::execute(image, request),
        Architecture::Avx2
        | Architecture::Avx
        | Architecture::Sse2
        | Architecture::Neon
        | Architecture::WasmSimd128 => super::helpers::execute_box_blur_radius_one(image),
        _ => Err(PilError::InternalError(
            "filter selected an unowned L BoxBlur architecture".into(),
        )),
    }
}

fn execute_gaussian_blur(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let super::super::ops::FilterRequest::GaussianBlur { sigma } = request else {
        return super::default::execute(image, request);
    };
    match selection.implementation.architecture {
        Architecture::Default => super::default::execute(image, request),
        Architecture::Avx2
        | Architecture::Avx
        | Architecture::Sse2
        | Architecture::Neon
        | Architecture::WasmSimd128 => {
            super::helpers::execute_gaussian_blur_radius_one(image, sigma)
        }
        _ => Err(PilError::InternalError(
            "filter selected an unowned L GaussianBlur architecture".into(),
        )),
    }
}

fn execute_filter5x5(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.architecture != Architecture::Default {
        if let Some(output) = execute_filter5x5_small_uniform(image, request)? {
            return Ok(output);
        }
    }
    match selection.implementation.architecture {
        Architecture::Default => super::default::execute(image, request),
        Architecture::Sse2
        | Architecture::Avx
        | Architecture::AvxFma
        | Architecture::Avx2
        | Architecture::Avx2Fma
        | Architecture::Neon
        | Architecture::WasmSimd128 => execute_filter5x5_vectorized(image, request),
        _ => Err(PilError::InternalError(
            "filter selected an unowned Filter5x5 architecture".into(),
        )),
    }
}

fn execute_filter3x3(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
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
    }
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
