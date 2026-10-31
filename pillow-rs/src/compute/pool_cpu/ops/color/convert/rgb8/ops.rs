//! RGB8 source-mode support and architecture selection for Convert.

use crate::compute::{Architecture, ExecutionContext, registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OperationInputs,
    OutputModeRule, StrategyGuard, StrategySelection,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage};

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));

const TARGETS: &[Mode] = &[
    Mode::ImageLuma8,
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
    Mode::ImageRgb8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageRgbx8,
    Mode::ImageCmyk8,
    Mode::ImageYCbCr8,
    Mode::ImageHsv8,
    Mode::ImageI32,
    Mode::ImageF32,
];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: TARGETS,
    core_only_targets: &[
        Mode::ImageLaPremultiplied8,
        Mode::ImageRgbPremultiplied8,
        Mode::ImageRgbx8,
    ],
    core_only_reason: Some("RGB to La, RGBa, and RGBX extend the archived Pillow Convert contract"),
    rejection_reason: "image has wrong mode",
}];

static ARCHITECTURES: [ImplementationSupport; 27] = [
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Avx2,
        cfg!(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "avx2"))),
        if cfg!(any(target_arch = "x86_64", target_feature = "ssse3")) {
            &[Architecture::Sse2, Architecture::Ssse3, Architecture::Avx2]
        } else {
            &[Architecture::Sse2, Architecture::Avx2]
        },
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Ssse3,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "ssse3",
            not(target_feature = "avx2")
        )),
        &[Architecture::Sse2, Architecture::Ssse3],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2",
            not(target_feature = "ssse3"),
            not(target_feature = "avx2")
        )),
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageLumaA8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageLumaA8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageLaPremultiplied8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageLaPremultiplied8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageCmyk8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageCmyk8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageYCbCr8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageYCbCr8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageHsv8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageHsv8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageI32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageI32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageF32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageF32),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgba8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgba8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgbPremultiplied8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgbPremultiplied8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgbx8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgb8,
        Architecture::Neon,
        cfg!(target_arch = "aarch64"),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgbx8),
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageRgb8,
    OutputModeRule::ByTargetMode,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) struct Prepared<'a> {
    source: &'a RgbImage,
    target: Mode,
    selection: StrategySelection,
}

impl Prepared<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        // SAFETY: prepare selected this strategy from SUPPORT using the same
        // source, target, and execution context retained in this plan.
        unsafe { execute(self.source, self.target, self.selection) }
    }
}

pub(crate) fn prepare<'a>(
    input: OperationInputs<'a>,
    target: Mode,
    ctx: &ExecutionContext,
) -> Result<Prepared<'a>, PilError> {
    let DynamicImage::ImageRgb8(source) = input.image else {
        return Err(PilError::InternalError(
            "RGB8 Convert strategy received another source mode".into(),
        ));
    };
    let selection = SUPPORT.select(ctx, input.facts().with_convert_target(target))?;
    Ok(Prepared { source, target, selection })
}

/// Executes the concrete RGB8 architecture strategy selected from `SUPPORT`.
///
/// # Safety
///
/// `selection` must come from [`SUPPORT`] for this RGB8 source and target,
/// with architecture capabilities checked by the same execution context.
pub(crate) unsafe fn execute(
    source: &RgbImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        #[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "sse2")))]
        Architecture::Sse2 => super::sse2::convert_impl::execute(source, target),
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "ssse3",
            not(target_feature = "avx2")
        ))]
        Architecture::Ssse3 if target == Mode::ImageLuma8 => {
            super::ssse3::convert_impl::execute(source)
        }
        #[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "avx2")))]
        Architecture::Avx2 if target == Mode::ImageLuma8 => {
            // SAFETY: SUPPORT admits this route only with its AVX2 requirements.
            unsafe { super::avx2::convert_impl::execute(source) }
        }
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 if target == Mode::ImageLuma8 => {
            super::wasm_simd128::convert_impl::execute(source)
        }
        #[cfg(target_arch = "aarch64")]
        Architecture::Neon => {
            // SAFETY: SUPPORT admits NEON only for the current aarch64 execution context.
            unsafe { super::neon::convert_impl::execute(source, target) }
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGB8 architecture".into(),
        )),
    }
}
