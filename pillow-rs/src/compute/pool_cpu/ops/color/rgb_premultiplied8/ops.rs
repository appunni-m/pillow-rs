//! RGBa source-mode support and architecture selection for Convert.

crate::compute::pool_cpu::ops::color::helpers::convert::binary::declare_mode_strategy!(
    crate::raster::Mode::ImageRgbPremultiplied8
);

use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, InputContract, ModeSupport, OperationId, StrategyGuard,
        StrategySelection,
    },
};
use crate::error::PilError;
use crate::pipeline::DitherMethod;
use crate::raster::{DynamicImage, Mode, RgbaImage};

const SSE2_AVAILABLE: bool = cfg!(any(
    target_arch = "x86_64",
    all(target_arch = "x86", target_feature = "sse2")
));
const WASM_SIMD128_AVAILABLE: bool = cfg!(all(target_arch = "wasm32", target_feature = "simd128"));

macro_rules! architecture_targets {
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

static ARCHITECTURES: [ImplementationSupport; 35] = architecture_targets!(
    ImageRgbPremultiplied8,
    singles [
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgbPremultiplied8,
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
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgbPremultiplied8,
        Architecture::Ssse3,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "ssse3",
            not(target_feature = "avx2")
        )),
        &[Architecture::Sse2, Architecture::Ssse3],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgbPremultiplied8,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2",
            not(target_feature = "ssse3"),
            not(target_feature = "avx2")
        )),
        &[Architecture::Sse2],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgbPremultiplied8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLuma8),
    ),
    ImplementationSupport::cpu(
        OperationId::Convert,
        Mode::ImageRgbPremultiplied8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ConvertTargetModeNonEmpty(Mode::ImageLuma8),
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
        ],
        ConvertTargetMode, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon]; [
            ImageLumaA8,
            ImageCmyk8,
            ImageYCbCr8,
            ImageHsv8,
            ImageI32,
            ImageF32,
        ],
        ConvertTargetModeNonEmpty, Sse2, SSE2_AVAILABLE, &[Architecture::Sse2]; [
            ImageLaPremultiplied8,
            ImageRgb8,
            ImageRgba8,
            ImageRgbx8,
        ],
        ConvertTargetModeNonEmpty, Neon, cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon]; [
            ImageLaPremultiplied8,
            ImageRgb8,
            ImageRgba8,
            ImageRgbx8,
        ],
        ConvertTargetMode, WasmSimd128, WASM_SIMD128_AVAILABLE, &[Architecture::WasmSimd128]; [
            ImageLumaA8,
            ImageCmyk8,
            ImageYCbCr8,
            ImageHsv8,
            ImageI32,
            ImageF32,
        ],
        ConvertTargetModeNonEmpty, WasmSimd128, WASM_SIMD128_AVAILABLE, &[Architecture::WasmSimd128]; [
            ImageLaPremultiplied8,
            ImageRgb8,
            ImageRgba8,
            ImageRgbx8,
        ],
    ]
);

// This owner computes each supported target directly. LAB reads unpremultiplied
// RGB samples; P/PA use their dedicated palette strategy.
const CORE_ONLY_TARGETS: &[Mode] = &[];
const INPUTS: &[InputContract] = &[InputContract::ConvertTarget {
    targets: super::super::ops::RGB_PREMULTIPLIED_TARGETS,
    core_only_targets: CORE_ONLY_TARGETS,
    core_only_reason: None,
    rejection_reason: "image has wrong mode",
}];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Convert,
    Mode::ImageRgbPremultiplied8,
    crate::compute::registry::support::OutputModeRule::ByTargetModeWithPalette,
    INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn execute_palette_target(
    source: &RgbaImage,
    target: Mode,
    dither: Option<DitherMethod>,
    selection: StrategySelection,
) -> Result<super::super::ops::PreparedConvertOutput, PilError> {
    match selection.implementation.architecture {
        Architecture::Default => {
            super::default::convert_impl::execute_palette_target(source, target, dither)
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGBa palette architecture".into(),
        )),
    }
}

pub(crate) fn execute(
    source: &RgbaImage,
    target: Mode,
    selection: StrategySelection,
) -> Result<DynamicImage, PilError> {
    let mut output = match selection.implementation.architecture {
        Architecture::Default => super::default::convert_impl::execute(source, target),
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "avx2"
        ))]
        Architecture::Avx2 if target == Mode::ImageLuma8 => {
            super::avx2::convert_impl::execute(source)
        }
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "ssse3",
            not(target_feature = "avx2")
        ))]
        Architecture::Ssse3 if target == Mode::ImageLuma8 => {
            super::ssse3::convert_impl::execute(source)
        }
        #[cfg(any(
            target_arch = "x86_64",
            all(target_arch = "x86", target_feature = "sse2")
        ))]
        Architecture::Sse2 => super::sse2::convert_impl::execute(source, target),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => super::wasm_simd128::convert_impl::execute(source, target),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => super::neon::convert_impl::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGBa architecture".into(),
        )),
    }?;
    // Pillow-compatible RGBa-to-YCbCr/HSV outputs have opaque physical X;
    // other RGB-family conversions retain the original fourth lane.
    if !matches!(target, Mode::ImageYCbCr8 | Mode::ImageHsv8) {
        super::super::helpers::convert::physical_lanes::preserve_fourth_byte(
            source.as_raw(),
            source.width(),
            source.height(),
            target,
            &mut output,
        )?;
    }
    Ok(output)
}
