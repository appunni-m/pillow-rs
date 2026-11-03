use super::super::ops::inputs_for;
use crate::compute::Architecture;
use crate::compute::registry::support::{
    F32RankCompatibility, ImplementationSupport, ModeSupport, OperationId, OutputModeRule,
    RejectionKind, StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

// TODO(pillow-rs#brg0): Add exact F32 NaN and mixed signed-zero parity cases
// for each rank operation and architecture. Default mirrors Pillow's serial
// partition comparisons; the content guard keeps these samples off vector paths.
const MODE: Mode = Mode::ImageF32;
pub(in crate::compute::pool_cpu::ops::filter) const MAX_RANK_WINDOW: u32 =
    super::helpers::MAX_RANK_WINDOW;

pub(in crate::compute::pool_cpu::ops::filter) fn rank_compatibility(
    raw: &[u8],
) -> F32RankCompatibility {
    if super::helpers::requires_default_comparator(raw) {
        F32RankCompatibility::DefaultComparator
    } else {
        F32RankCompatibility::VectorOrdering
    }
}

macro_rules! rank_vector_architectures {
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
                StrategyGuard::FilterF32RankWindow { max_size: MAX_RANK_WINDOW },
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
                StrategyGuard::FilterF32RankWindow { max_size: MAX_RANK_WINDOW },
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
                StrategyGuard::FilterF32RankWindow { max_size: MAX_RANK_WINDOW },
            ),
            ImplementationSupport::cpu(
                $operation,
                MODE,
                Architecture::Neon,
                cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                &[Architecture::Neon],
                StrategyGuard::FilterF32RankWindow { max_size: MAX_RANK_WINDOW },
            ),
            ImplementationSupport::cpu(
                $operation,
                MODE,
                Architecture::WasmSimd128,
                cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                &[Architecture::WasmSimd128],
                StrategyGuard::FilterF32RankWindow { max_size: MAX_RANK_WINDOW },
            ),
        ];
    };
}

rank_vector_architectures!(MEDIAN_ARCHITECTURES, OperationId::MedianFilter);
rank_vector_architectures!(MAX_ARCHITECTURES, OperationId::MaxFilter);
rank_vector_architectures!(MIN_ARCHITECTURES, OperationId::MinFilter);
rank_vector_architectures!(RANK_ARCHITECTURES, OperationId::RankFilter);

fn rejected(mode: Mode) -> ModeSupport {
    ModeSupport::Rejected { mode, reason: "image has wrong mode", kind: RejectionKind::ValueError }
}

pub(in crate::compute::pool_cpu::ops::filter) fn support(operation: OperationId) -> ModeSupport {
    match operation {
        OperationId::Filter3x3 => rejected(Mode::ImageF32),
        OperationId::Filter5x5 => rejected(Mode::ImageF32),
        OperationId::GaussianBlur => rejected(Mode::ImageF32),
        OperationId::BoxBlur => rejected(Mode::ImageF32),
        OperationId::MedianFilter => ModeSupport::cpu(
            OperationId::MedianFilter,
            MODE,
            OutputModeRule::Preserve,
            inputs_for(OperationId::MedianFilter),
            MEDIAN_ARCHITECTURES,
        ),
        OperationId::MaxFilter => ModeSupport::cpu(
            OperationId::MaxFilter,
            MODE,
            OutputModeRule::Preserve,
            inputs_for(OperationId::MaxFilter),
            MAX_ARCHITECTURES,
        ),
        OperationId::MinFilter => ModeSupport::cpu(
            OperationId::MinFilter,
            MODE,
            OutputModeRule::Preserve,
            inputs_for(OperationId::MinFilter),
            MIN_ARCHITECTURES,
        ),
        OperationId::RankFilter => ModeSupport::cpu(
            OperationId::RankFilter,
            MODE,
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

pub(in crate::compute::pool_cpu::ops::filter) fn execute(
    image: &DynamicImage,
    request: super::super::ops::FilterRequest,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let (size, rank) = match request {
        super::super::ops::FilterRequest::MedianFilter { size } => {
            (size, size.saturating_mul(size) / 2)
        }
        super::super::ops::FilterRequest::MaxFilter { size } => {
            (size, size.saturating_mul(size).saturating_sub(1))
        }
        super::super::ops::FilterRequest::MinFilter { size } => (size, 0),
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
            let DynamicImage::ImageF32(input) = image else {
                return Err(PilError::InternalError(
                    "F rank strategy received a mismatched raster variant".into(),
                ));
            };
            let output = super::helpers::execute_rank_vector(
                input.as_raw(),
                input.width(),
                input.height(),
                size,
                rank,
            )?;
            crate::raster::RgbaImage::from_raw(input.width(), input.height(), output)
                .map(DynamicImage::ImageF32)
                .ok_or_else(|| {
                    PilError::ValueError("rank output buffer does not match F mode".into())
                })
        }
        _ => Err(PilError::InternalError("filter selected an unowned F architecture".into())),
    }
}
