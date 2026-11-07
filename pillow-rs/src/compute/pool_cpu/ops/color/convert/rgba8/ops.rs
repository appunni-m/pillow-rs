//! RGBA8 source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::convert::helpers::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageRgba8
);

use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
        StrategyGuard, StrategySelection,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));
const WASM_SIMD128_AVAILABLE: bool = cfg!(all(target_arch = "wasm32", target_feature = "simd128"));

macro_rules! architecture_catalog {
    (
        $mode:ident,
        singles [$($single:expr),* $(,)?],
        groups [
            $(
                $guard:ident, $architecture:ident, $available:expr, $requirements:expr;
                [$($target:ident),+ $(,)?]
            ),+ $(,)?
        ]
    ) => {
        [
            $($single,)*
            $(
                $(
                    ImplementationSupport::cpu(
                        OperationId::Convert,
                        Mode::$mode,
                        Architecture::$architecture,
                        $available,
                        $requirements,
                        StrategyGuard::$guard(Mode::$target),
                    ),
                )+
            )+
        ]
    };
}

static ARCHITECTURES: [ImplementationSupport; 36] = architecture_catalog!(
    ImageRgba8,
    singles [
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Ssse3,
        cfg!(target_arch = "x86_64"),
        &[Architecture::Ssse3],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetMode(Mode::ImageRgb8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgbx8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgbx8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Sse2,
        SSE2_AVAILABLE,
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgbPremultiplied8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageRgbPremultiplied8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Avx2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx2"
        )),
        if cfg!(target_feature = "ssse3") {
            &[Architecture::Sse2, Architecture::Ssse3, Architecture::Avx2]
        } else {
            &[Architecture::Sse2, Architecture::Avx2]
        },
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
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
        Mode::ImageRgba8,
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
        Mode::ImageRgba8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgba8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetMode(Mode::ImageLuma8),
    ),
    ],
    groups [
        ConvertTargetMode, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]; [
            ImageLumaA8,
            ImageCmyk8,
            ImageYCbCr8,
            ImageHsv8,
            ImageI32,
            ImageF32,
            ImageRgba8,
        ],
        ConvertTargetMode, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon]; [
            ImageLumaA8,
            ImageCmyk8,
            ImageYCbCr8,
            ImageHsv8,
            ImageI32,
            ImageF32,
            ImageRgba8,
        ],
        ConvertTargetModeNonEmpty, WasmSimd128, WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128]; [ImageRgbx8],
        ConvertTargetModeNonEmpty, WasmSimd128, WASM_SIMD128_AVAILABLE,
        &[Architecture::WasmSimd128]; [ImageRgbPremultiplied8],
        ConvertTargetMode, WasmSimd128, WASM_SIMD128_AVAILABLE, &[Architecture::WasmSimd128]; [
            ImageRgb8,
            ImageRgba8,
            ImageLumaA8,
            ImageCmyk8,
            ImageYCbCr8,
            ImageHsv8,
            ImageI32,
            ImageF32,
        ],
    ]
);

// RGBA-to-LAB reads RGB components directly; alpha is not part of LAB output.
// Pillow's wrapper routes missing direct RGBA targets through RGB.
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: super::super::ops::RGBA_TARGETS,
    core_only_targets: &[],
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageRgba8,
    OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn execute_palette_target(
    source: &RgbaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGBA palette architecture".into(),
        )),
    }
}

/// Executes the RGBA8 strategy selected from this mode's support declaration.
///
/// # Safety
///
/// `selection` must come from [`SUPPORT`] for this source and target, with the
/// selected architecture's requirements checked by the same execution context.
#[allow(
    unsafe_code,
    reason = "architecture dispatch relies on the validated Convert strategy selection"
)]
pub(crate) unsafe fn execute(
    source: &RgbaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    let mut output = match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "avx2"))]
        Architecture::Avx2 if target == Mode::ImageLuma8 => {
            super::avx2::convert_impl::execute(source)
        }
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "ssse3",
            not(target_feature = "avx2")
        ))]
        Architecture::Ssse3 if target == Mode::ImageLuma8 => {
            super::ssse3::convert_impl::execute_luma(source)
        }
        #[cfg(any(target_arch = "x86_64", all(target_arch = "x86", target_feature = "sse2")))]
        Architecture::Sse2 => super::sse2::convert_impl::execute(source, target),
        #[cfg(target_arch = "x86_64")]
        Architecture::Ssse3 if target == Mode::ImageRgb8 => {
            // SAFETY: SUPPORT binds this route to the SSSE3 RGBA kernel.
            unsafe { super::ssse3::convert_impl::execute(source) }
        }
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => super::neon::convert_impl::execute(source, target),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => {
            super::wasm_simd128::convert_impl::execute(source, target)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGBA architecture".into(),
        )),
    }?;
    super::super::helpers::physical_lanes::preserve_fourth_byte(
        source.as_raw(),
        source.width(),
        source.height(),
        target,
        &mut output,
    )?;
    Ok(output)
}
