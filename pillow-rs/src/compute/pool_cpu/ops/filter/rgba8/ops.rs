use super::super::ops::{core_only, core_only_serial, inputs_for, pillow_kernel};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) const MODE: Mode = Mode::ImageRgba8;

static MAX_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MaxFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 4 },
);
static MIN_FILTER_ARCHITECTURES:  [crate::compute::registry::support::ImplementationSupport; 5] = crate::compute::pool_cpu::ops::filter::helpers::native_byte_filter_architectures(
    OperationId::MinFilter, MODE,
    crate::compute::registry::support::StrategyGuard::FilterNativeByteExtrema { channels: 4 },
);
// TODO(pillow-rs#3ptb): freeze RGBA radius-seven GaussianBlur output and
// selected/fallback receipts for Default, AVX2, and NEON after CPU source closure.
static GAUSSIAN_BLUR_ARCHITECTURES: [ImplementationSupport; 7] = [
    ImplementationSupport::cpu(
        OperationId::GaussianBlur,
        MODE,
        Architecture::Avx2,
        cfg!(target_arch = "x86_64"),
        &[Architecture::Avx2],
        StrategyGuard::FilterRgbaGaussianBlurRadiusSeven,
    ),
    ImplementationSupport::cpu(
        OperationId::GaussianBlur,
        MODE,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::FilterRgbaGaussianBlurRadiusSeven,
    ),
    ImplementationSupport::cpu(
        OperationId::GaussianBlur,
        MODE,
        Architecture::Avx2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx2"
        )),
        &[Architecture::Avx2],
        StrategyGuard::FilterNativeByteBlur { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::GaussianBlur,
        MODE,
        Architecture::Avx,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx",
            not(target_feature = "avx2")
        )),
        &[Architecture::Avx],
        StrategyGuard::FilterNativeByteBlur { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::GaussianBlur,
        MODE,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2",
            not(target_feature = "avx")
        )),
        &[Architecture::Sse2],
        StrategyGuard::FilterNativeByteBlur { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::GaussianBlur,
        MODE,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::FilterNativeByteBlur { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::GaussianBlur,
        MODE,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::FilterNativeByteBlur { channels: 4 },
    ),
];
static BOX_BLUR_ARCHITECTURES: [ImplementationSupport; 5] =
    crate::compute::pool_cpu::ops::filter::helpers::native_byte_blur_architectures(
        OperationId::BoxBlur,
        MODE,
        4,
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
crate::compute::pool_cpu::ops::filter::helpers::native_filter3x3_arch_leaf!(ImageRgba8, RgbaImage, 4);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_architectures!(MODE, 4);
crate::compute::pool_cpu::ops::filter::helpers::native_filter5x5_arch_leaf!(ImageRgba8, RgbaImage, 4);


pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        OperationId::Filter3x3 => pillow_kernel(OperationId::Filter3x3, MODE, FILTER3X3_ARCHITECTURES),
        OperationId::Filter5x5 => pillow_kernel(OperationId::Filter5x5, MODE, FILTER5X5_ARCHITECTURES),
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
    crate::compute::pool_cpu::ops::filter::helpers::validate_native_byte_blur_selection(
        selection, context,
    )?;
    if !matches!(image, DynamicImage::ImageRgba8(_)) {
        return Err(PilError::InternalError(
            "RGBA blur strategy received a mismatched raster variant".into(),
        ));
    }
    if let super::super::ops::FilterRequest::GaussianBlur { sigma } = request {
        let parameters @ (radius, _, fractional_weight) =
            crate::compute::pool_cpu::ops::filter::helpers::gaussian_blur_box_parameters(sigma);
        if radius == 7 && fractional_weight != 0 {
            let evidence = super::helpers::prove_gaussian_blur_uniform_identity(
                image.as_bytes(),
                image.width() as usize,
                image.height() as usize,
                parameters,
                parameters,
            );
            if evidence.vector_blocks != 0 {
                crate::compute::record_pipeline_operation_path("vector");
                crate::compute::record_pipeline_operation_vector_blocks(evidence.vector_blocks);
                if evidence.scalar_tail_pixels != 0 {
                    crate::compute::record_pipeline_operation_scalar_tail(
                        evidence.scalar_tail_pixels,
                    );
                }
            }
            if evidence.identity {
                return crate::compute::pool_cpu::ops::filter::helpers::
                    copy_native_byte_blur_input(image, MODE, 4);
            }
        }
        if radius == 7
            && fractional_weight != 0
            && matches!(selection.implementation.architecture, Architecture::Avx2 | Architecture::Neon)
        {
            return execute_gaussian_blur(image, sigma, selection, context);
        }
    }
    if let Some(output) = super::helpers::try_execute_radius_one_blur(image, request)? {
        return Ok(output);
    }
    super::helpers::execute_native_byte_blur(
        image,
        MODE,
        4,
        request,
        selection.implementation.architecture,
        context,
    )
}

pub(in crate::compute::pool_cpu::ops::filter) fn execute(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
    context: &crate::compute::ExecutionContext,
) -> Result<DynamicImage, PilError> {
    if let super::super::ops::FilterRequest::GaussianBlur { sigma } = request {
        return execute_gaussian_blur(image, sigma, selection, context);
    }
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
    sigma: f32,
    selection: &StrategySelection,
    context: &crate::compute::ExecutionContext,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::execute(
            image,
            super::super::ops::FilterRequest::GaussianBlur { sigma },
        ),
        Architecture::Avx2 => {
            #[cfg(target_arch = "x86_64")]
            {
                super::avx2::execute_gaussian_blur_radius_seven(image, sigma, context)
            }
            #[cfg(not(target_arch = "x86_64"))]
            {
                Err(PilError::InternalError(
                    "RGBA GaussianBlur selected AVX2 on a non-x86_64 target".into(),
                ))
            }
        }
        Architecture::Neon => {
            #[cfg(target_arch = "aarch64")]
            {
                super::neon::execute_gaussian_blur_radius_seven(image, sigma, context)
            }
            #[cfg(not(target_arch = "aarch64"))]
            {
                Err(PilError::InternalError(
                    "RGBA GaussianBlur selected NEON on a non-AArch64 target".into(),
                ))
            }
        }
        _ => Err(PilError::InternalError(
            "RGBA GaussianBlur selected an unowned architecture".into(),
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
