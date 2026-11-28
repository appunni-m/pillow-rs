use super::super::ops::{inputs_for, pillow_kernel};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, RejectionKind, StrategyGuard,
    StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

pub(super) const MODE: Mode = Mode::ImageI32;
pub(super) const MAX_RANK_WINDOW: u32 = 9;
pub(super) const AVX512F_MINIMUM_WIDTH: u32 = 16;
pub(super) const AVX512F_MAX_PIXELS: u64 = i32::MAX as u64;

macro_rules! rank_avx512f_architectures {
    ($name:ident, $operation:expr) => {
        const $name: &[ImplementationSupport] = &[ImplementationSupport::cpu(
            $operation,
            MODE,
            Architecture::Avx512F,
            cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            &[Architecture::Avx512F],
            StrategyGuard::FilterI32Avx512RankWindow {
                max_size: MAX_RANK_WINDOW,
                minimum_width: AVX512F_MINIMUM_WIDTH,
                max_pixels: AVX512F_MAX_PIXELS,
            },
        )];
    };
}

rank_avx512f_architectures!(MEDIAN_ARCHITECTURES, OperationId::MedianFilter);
rank_avx512f_architectures!(MAX_ARCHITECTURES, OperationId::MaxFilter);
rank_avx512f_architectures!(MIN_ARCHITECTURES, OperationId::MinFilter);
rank_avx512f_architectures!(RANK_ARCHITECTURES, OperationId::RankFilter);

/// Prove an I-mode 3x3 identity with the selected mode and architecture strategy.
// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn prepared_uniform_filter3x3_identity(
    image: &DynamicImage,
    kernel: &[f32; 9],
    scale: f32,
    offset: f32,
    selection: &StrategySelection,
) -> bool {
    if image.mode() != MODE {
        return false;
    }
    let normalized = std::array::from_fn(|index| kernel[index] / scale);
    match selection.implementation.architecture {
        Architecture::Default => super::default::uniform_i32_filter3x3_is_identity(
            image.as_bytes(),
            image.width() as usize,
            image.height() as usize,
            &normalized,
            offset + 0.5,
        ),
        Architecture::Avx2Fma => super::avx2::uniform_i32_filter3x3_is_identity(
            image.as_bytes(),
            image.width() as usize,
            image.height() as usize,
            &normalized,
            offset + 0.5,
        ),
        Architecture::Avx512F => super::avx2::uniform_i32_filter3x3_is_identity(
            image.as_bytes(),
            image.width() as usize,
            image.height() as usize,
            &normalized,
            offset + 0.5,
        ),
        _ => false,
    }
}

const FILTER3X3_ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::Filter3x3,
        MODE,
        Architecture::Avx512F,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        &[Architecture::Avx512F],
        StrategyGuard::Filter3x3I32Avx512F,
    ),
    ImplementationSupport::cpu(
        OperationId::Filter3x3,
        MODE,
        Architecture::Avx2Fma,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx2",
            target_feature = "fma"
        )),
        &[Architecture::Avx2Fma],
        StrategyGuard::Filter3x3I32Vectorizable { channels: 4 },
    ),
];

const FILTER5X5_ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::Filter5x5,
        MODE,
        Architecture::Avx512F,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        &[Architecture::Avx512F],
        StrategyGuard::Filter5x5I32Avx512F,
    ),
    ImplementationSupport::cpu(
        OperationId::Filter5x5,
        MODE,
        Architecture::Avx2Fma,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx2",
            target_feature = "fma"
        )),
        &[Architecture::Avx2Fma],
        StrategyGuard::Filter5x5I32Vectorizable,
    ),
    ImplementationSupport::cpu(
        OperationId::Filter5x5,
        MODE,
        Architecture::AvxFma,
        cfg!(any(target_arch = "x86", target_arch = "x86_64")),
        &[Architecture::AvxFma],
        StrategyGuard::Filter5x5I32BinomialFma,
    ),
    ImplementationSupport::cpu(
        OperationId::Filter5x5,
        MODE,
        Architecture::X86_64,
        cfg!(target_arch = "x86_64"),
        &[],
        StrategyGuard::Always,
    ),
];

// TODO(pillow-rs#3ptb): freeze I32 Filter3x3/Filter5x5 architecture and
// fallback receipts, including the guarded AVX/FMA Filter5x5 route. For
// Filter3x3, the guard admits nonempty shapes with no complete four-lane
// interior block; verify whether AVX2/FMA scalar-tail and no-interior identity
// paths are intended, including identity-sharing receipts and latency.
// TODO(pillow-rs#fvsp): verify AVX-512F I32 rank/convolution outputs and fallback receipts.

fn rejected() -> ModeSupport {
    ModeSupport::Rejected {
        mode: MODE,
        reason: "image has wrong mode",
        kind: RejectionKind::ValueError,
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        OperationId::Filter3x3 => {
            pillow_kernel(OperationId::Filter3x3, MODE, FILTER3X3_ARCHITECTURES)
        }
        OperationId::Filter5x5 => {
            pillow_kernel(OperationId::Filter5x5, MODE, FILTER5X5_ARCHITECTURES)
        }
        OperationId::GaussianBlur | OperationId::BoxBlur | OperationId::BoxBlurXY => rejected(),
        OperationId::MedianFilter => ModeSupport::cpu(
            OperationId::MedianFilter,
            Mode::ImageI32,
            OutputModeRule::Preserve,
            inputs_for(OperationId::MedianFilter),
            MEDIAN_ARCHITECTURES,
        ),
        OperationId::MaxFilter => ModeSupport::cpu(
            OperationId::MaxFilter,
            Mode::ImageI32,
            OutputModeRule::Preserve,
            inputs_for(OperationId::MaxFilter),
            MAX_ARCHITECTURES,
        ),
        OperationId::MinFilter => ModeSupport::cpu(
            OperationId::MinFilter,
            Mode::ImageI32,
            OutputModeRule::Preserve,
            inputs_for(OperationId::MinFilter),
            MIN_ARCHITECTURES,
        ),
        OperationId::RankFilter => ModeSupport::cpu(
            OperationId::RankFilter,
            Mode::ImageI32,
            OutputModeRule::Preserve,
            inputs_for(OperationId::RankFilter),
            RANK_ARCHITECTURES,
        ),
        _ => ModeSupport::Rejected {
            mode: MODE,
            reason: "non-filter operation reached filter mode strategy",
            kind: crate::compute::registry::support::RejectionKind::InternalError,
        },
    }
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn rank_window(request: &super::super::ops::FilterRequest) -> Option<(u32, u32, OperationId)> {
    match request {
        super::super::ops::FilterRequest::MedianFilter { size } => Some((
            *size,
            size.saturating_mul(*size) / 2,
            OperationId::MedianFilter,
        )),
        super::super::ops::FilterRequest::MaxFilter { size } => Some((
            *size,
            size.saturating_mul(*size).saturating_sub(1),
            OperationId::MaxFilter,
        )),
        super::super::ops::FilterRequest::MinFilter { size } => {
            Some((*size, 0, OperationId::MinFilter))
        }
        super::super::ops::FilterRequest::RankFilter { size, rank } => {
            Some((*size, *rank, OperationId::RankFilter))
        }
        _ => None,
    }
}

fn has_no_interior(request: &super::super::ops::FilterRequest, width: u32, height: u32) -> bool {
    match request {
        super::super::ops::FilterRequest::Filter3x3 { .. } => width <= 2 || height <= 2,
        super::super::ops::FilterRequest::Filter5x5 { .. } => width <= 4 || height <= 4,
        _ => false,
    }
}

fn execute_no_interior_identity(image: &DynamicImage) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI32(source) = image else {
        return Err(PilError::InternalError(
            "I-mode filter identity received a non-I raster".into(),
        ));
    };
    let output =
        crate::compute::pool_cpu::ops::filter::helpers::execute_native_filter_identity_copy(
            source.as_raw(),
        )?;
    let output = RgbaImage::from_raw(source.width(), source.height(), output).ok_or_else(|| {
        PilError::InternalError("I-mode filter identity output does not match its mode".into())
    })?;
    Ok(DynamicImage::ImageI32(output))
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::filter) fn execute(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    if selection.implementation.architecture != Architecture::Default
        && has_no_interior(&request, image.width(), image.height())
    {
        return execute_no_interior_identity(image);
    }
    match selection.implementation.architecture {
        Architecture::Default => super::default::execute(image, request),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Architecture::Avx512F => {
            let DynamicImage::ImageI32(input) = image else {
                return Err(PilError::InternalError(
                    "I AVX-512F filter strategy received a mismatched raster variant".into(),
                ));
            };
            match request {
                super::super::ops::FilterRequest::Filter3x3 { .. }
                | super::super::ops::FilterRequest::Filter5x5 { .. } => {
                    super::avx512f::convolution_impl::execute(input, request, selection)
                }
                _ => {
                    let Some((size, rank, operation)) = rank_window(&request) else {
                        return Err(PilError::InternalError(
                            "AVX-512F rank strategy received a non-rank request".into(),
                        ));
                    };
                    super::avx512f::rank_impl::execute(input, size, rank, operation, selection)
                }
            }
        }
        Architecture::Avx2Fma => match request {
            request @ super::super::ops::FilterRequest::Filter3x3 { .. } => {
                super::avx2::execute_filter3x3(image, request)
            }
            super::super::ops::FilterRequest::Filter5x5 {
                kernel,
                scale,
                offset,
            } => super::avx2::execute(image, &kernel, scale, offset),
            _ => Err(PilError::InternalError(
                "AVX2/FMA filter selection received an unsupported convolution request".into(),
            )),
        },
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Architecture::AvxFma => match request {
            super::super::ops::FilterRequest::Filter5x5 {
                kernel,
                scale,
                offset,
            } => super::avx::execute(image, &kernel, scale, offset),
            _ => Err(PilError::InternalError(
                "AVX/FMA filter selection received an unsupported convolution request".into(),
            )),
        },
        #[cfg(target_arch = "x86_64")]
        Architecture::X86_64 => match request {
            super::super::ops::FilterRequest::Filter5x5 {
                kernel,
                scale,
                offset,
            } => super::x86_64::execute(image, &kernel, scale, offset),
            _ => Err(PilError::InternalError(
                "x86_64 filter selection received an unsupported request".into(),
            )),
        },
        _ => Err(PilError::InternalError(
            "filter selected an unowned architecture".into(),
        )),
    }
}
