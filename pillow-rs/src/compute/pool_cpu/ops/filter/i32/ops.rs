use super::super::ops::{core_only, inputs_for};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule, RejectionKind,
    StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

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
        Architecture::Default => default::uniform_i32_filter3x3_is_identity(
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

const AVX2_FMA_FILTER5X5: &[ImplementationSupport] = &[ImplementationSupport::cpu(
    OperationId::Filter5x5,
    MODE,
    Architecture::Avx2Fma,
    cfg!(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "avx2",
        target_feature = "fma"
    )),
    &[Architecture::Avx2Fma],
    StrategyGuard::Always,
)];

fn rejected() -> ModeSupport {
    ModeSupport::Rejected {
        mode: MODE,
        reason: "image has wrong mode",
        kind: RejectionKind::ValueError,
    }
}


pub(super) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        // NO_PILLOW_SUPPORT: Pillow 12.2 Kernel is documented for L and RGB only;
        // this native mode strategy extends that contract.
        OperationId::Filter3x3 => core_only(OperationId::Filter3x3, MODE, AVX2_FMA_FILTER3X3),
        // NO_PILLOW_SUPPORT: Pillow 12.2 Kernel is documented for L and RGB only;
        // this native mode strategy extends that contract.
        OperationId::Filter5x5 => core_only(OperationId::Filter5x5, MODE, AVX2_FMA_FILTER5X5),
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

pub(super) fn execute(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => default::execute(image, request),
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
        _ => Err(PilError::InternalError("filter selected an unowned architecture".into())),
    }
}
