use super::super::ops::{core_only, core_only_serial, inputs_for};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::ImageRgb8;

static MAX_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = super::super::helpers::native_byte_filter_architectures(
    OperationId::MaxFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 3 },
);
static MIN_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = super::super::helpers::native_byte_filter_architectures(
    OperationId::MinFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 3 },
);
static GAUSSIAN_BLUR_ARCHITECTURES: [ImplementationSupport; 5] =
    super::super::helpers::native_byte_blur_architectures(
        OperationId::GaussianBlur,
        MODE,
        3,
    );
static BOX_BLUR_ARCHITECTURES: [ImplementationSupport; 5] =
    super::super::helpers::native_byte_blur_architectures(
        OperationId::BoxBlur,
        MODE,
        3,
    );
static MEDIAN_FILTER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] = super::super::helpers::native_byte_filter_architectures(
    OperationId::MedianFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteRankWindow { channels: 3 },
);
static RANK_FILTER_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 5] = super::super::helpers::native_byte_filter_architectures(
    OperationId::RankFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteRankWindow { channels: 3 },
);

super::super::helpers::native_filter3x3_architectures!(MODE, 3);
super::super::helpers::native_filter3x3_arch_leaf!(ImageRgb8, RgbImage, 3);
super::super::helpers::native_filter5x5_architectures!(MODE, 3);
super::super::helpers::native_filter5x5_arch_leaf!(ImageRgb8, RgbImage, 3, rgb);

const UNSHARP_ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::UnsharpMask,
        MODE,
        Architecture::Sse2,
        cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
        &[Architecture::Sse2],
        StrategyGuard::FilterUnsharpRgb150,
    ),
    ImplementationSupport::cpu(
        OperationId::UnsharpMask,
        MODE,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::FilterUnsharpRgb150,
    ),
    ImplementationSupport::cpu(
        OperationId::UnsharpMask,
        MODE,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::FilterUnsharpRgb150,
    ),
];

pub(in crate::compute::pool_cpu::ops::filter) const fn unsharp_mask_support() -> ModeSupport {
    ModeSupport::cpu(
        OperationId::UnsharpMask,
        MODE,
        OutputModeRule::Preserve,
        inputs_for(OperationId::UnsharpMask),
        UNSHARP_ARCHITECTURES,
    )
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute_unsharp_blend(
    selection: &StrategySelection,
    original: &[u8],
    blurred_output: &mut [u8],
    threshold: i32,
) -> bool {
    match selection.implementation.architecture {
        Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128 => {
            super::helpers::execute_unsharp_rgb150(original, blurred_output, threshold)
        }
        Architecture::Default => false,
        _ => false,
    }
}


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
    super::super::helpers::validate_native_byte_blur_selection(
        selection, MODE, request, context,
    )?;
    match request {
        super::super::ops::FilterRequest::BoxBlur { radius: 1 } => {
            super::helpers::execute_box_blur_radius_one(image, request)
        }
        super::super::ops::FilterRequest::BoxBlurXY { radius_x, radius_y, passes }
            if radius_x == 1.0 && radius_y == 1.0 && passes == 1 =>
        {
            super::helpers::execute_box_blur_radius_one(image, request)
        }
        super::super::ops::FilterRequest::GaussianBlur { sigma }
            if (1.0..2.0).contains(
                &crate::compute::helpers::filters::gaussian_blur_three_pass_radius(
                    sigma.abs(),
                ),
            ) =>
        {
            super::helpers::execute_gaussian_blur_radius_one(image, sigma)
        }
        _ => super::super::helpers::execute_native_byte_blur(
            image,
            MODE,
            3,
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
    if selection.implementation.architecture != Architecture::Default
        && matches!(
            request,
            super::super::ops::FilterRequest::MaxFilter { .. }
                | super::super::ops::FilterRequest::MinFilter { .. }
        )
    {
        return super::super::helpers::execute_native_byte_extrema(
            image, request, MODE, 3, selection,
        );
    }

    if selection.implementation.architecture != Architecture::Default
        && matches!(
            request,
            super::super::ops::FilterRequest::MedianFilter { .. }
                | super::super::ops::FilterRequest::RankFilter { .. }
        )
    {
        return super::super::helpers::execute_native_byte_rank(
            image, request, MODE, 3, selection,
        );
    }

    if matches!(request, super::super::ops::FilterRequest::Filter5x5 { .. }) {
        if selection.implementation.architecture != Architecture::Default {
            if let Some(output) = execute_filter5x5_small_uniform(image, request)? {
                return Ok(output);
            }
        }
        return super::super::helpers::native_filter5x5_arch_dispatch!(
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
            "filter selected an unowned RGB GaussianBlur architecture".into(),
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
        | Architecture::WasmSimd128 => {
            super::helpers::execute_box_blur_radius_one(image, request)
        }
        _ => Err(PilError::InternalError(
            "filter selected an unowned RGB BoxBlur architecture".into(),
        )),
    }
}

pub(in crate::compute::pool_cpu::ops::filter) fn mode_filter_support() -> crate::compute::registry::support::ModeSupport {
    super::super::ops::core_only_serial(crate::compute::registry::support::OperationId::ModeFilter, MODE)
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute_mode_filter(
    image: &crate::raster::DynamicImage,
    size: u32,
) -> Result<crate::raster::DynamicImage, crate::error::PilError> {
    super::default::execute_mode_filter(image, size)
}
