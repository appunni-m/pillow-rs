use super::super::ops::{inputs_for, pillow_kernel};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, RejectionKind,
    StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

pub(super) const MODE: Mode = Mode::ImageI32;

/// Prove an I-mode 3x3 identity with the selected mode and architecture strategy.
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
        Architecture::Avx2Fma => avx2::uniform_i32_filter3x3_is_identity(
            image.as_bytes(),
            image.width() as usize,
            image.height() as usize,
            &normalized,
            offset + 0.5,
        ),
        _ => false,
    }
}

const AVX2_FMA_FILTER3X3: &[ImplementationSupport] = &[ImplementationSupport::cpu(
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
)];

const FILTER5X5_ARCHITECTURES: &[ImplementationSupport] = &[
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
];

// TODO(pillow-rs#3ptb): Freeze I32 Filter3x3/Filter5x5 output and
// architecture/fallback receipts in the CPU parity matrix, including the
// guarded AVX/FMA Filter5x5 route.

fn rejected() -> ModeSupport {
    ModeSupport::Rejected {
        mode: MODE,
        reason: "image has wrong mode",
        kind: RejectionKind::ValueError,
    }
}


pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        OperationId::Filter3x3 => pillow_kernel(OperationId::Filter3x3, MODE, AVX2_FMA_FILTER3X3),
        OperationId::Filter5x5 => pillow_kernel(OperationId::Filter5x5, MODE, FILTER5X5_ARCHITECTURES),
        OperationId::GaussianBlur | OperationId::BoxBlur => rejected(),
        OperationId::MedianFilter => ModeSupport::serial(
            OperationId::MedianFilter, Mode::ImageI32, inputs_for(OperationId::MedianFilter),
        ),
        OperationId::MaxFilter => ModeSupport::serial(
            OperationId::MaxFilter, Mode::ImageI32, inputs_for(OperationId::MaxFilter),
        ),
        OperationId::MinFilter => ModeSupport::serial(
            OperationId::MinFilter, Mode::ImageI32, inputs_for(OperationId::MinFilter),
        ),
        OperationId::RankFilter => ModeSupport::serial(
            OperationId::RankFilter, Mode::ImageI32, inputs_for(OperationId::RankFilter),
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
    let no_interior = match &request {
        super::super::ops::FilterRequest::Filter3x3 { .. } => {
            image.width() <= 2 || image.height() <= 2
        }
        super::super::ops::FilterRequest::Filter5x5 { .. } => {
            image.width() <= 4 || image.height() <= 4
        }
        _ => false,
    };
    if selection.implementation.architecture != Architecture::Default && no_interior {
        let DynamicImage::ImageI32(source) = image else {
            return Err(PilError::InternalError(
                "I-mode filter identity received a non-I raster".into(),
            ));
        };
        let output = crate::compute::pool_cpu::ops::filter::helpers::execute_native_filter_identity_copy(
            source.as_raw(),
        )?;
        let output = RgbaImage::from_raw(source.width(), source.height(), output).ok_or_else(|| {
            PilError::InternalError("I-mode filter identity output does not match its mode".into())
        })?;
        return Ok(DynamicImage::ImageI32(output));
    }
    match selection.implementation.architecture {
        Architecture::Default => super::default::execute(image, request),
        Architecture::Avx2Fma => match request {
            request @ super::super::ops::FilterRequest::Filter3x3 { .. } => {
                avx2::execute_filter3x3(image, request)
            }
            super::super::ops::FilterRequest::Filter5x5 { kernel, scale, offset } => {
                avx2::execute(image, &kernel, scale, offset)
            }
            _ => Err(PilError::InternalError(
                "AVX2/FMA filter selection received an unsupported convolution request".into(),
            )),
        },
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Architecture::AvxFma => match request {
            super::super::ops::FilterRequest::Filter5x5 { kernel, scale, offset } => {
                avx::execute(image, &kernel, scale, offset)
            }
            _ => Err(PilError::InternalError(
                "AVX/FMA filter selection received an unsupported convolution request".into(),
            )),
        },
        _ => Err(PilError::InternalError("filter selected an unowned architecture".into())),
    }
}
