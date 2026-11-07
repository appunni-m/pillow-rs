//! Resize operation contracts and prepared mode-strategy selection.
//!
//! Boxed Resize keeps Pillow's float32 bounds, crop/pass-elision, public
//! validation order, and vertical-first tall-image composition. Public and
//! nested geometry callers enter prepared plans with the injected context.
//! One and P normalize to nearest before mode and architecture selection;
//! their Default routes remain serial fallbacks when no vector route applies.
//! Rayon scheduling belongs to Parallel CPU.
//! TODO(pillow-rs#3ptb): verify exact-mode output, bounds/empty cases, and
//! requested-architecture fallback receipts across the frozen CPU matrix.
//! Resize/ResizeBoxed now compose omitted lanes at the prepared execution
//! boundary. Current fused Resize-to-Pad owners are L and RGBX; neither has
//! omitted physical lanes. Compact modes use prepared Resize followed by Pad.

use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, InputContract, ModeSupport, OperationId, OperationSupport,
        OutputModeRule, StrategyGuard,
    },
};
use crate::raster::Mode;

const RESIZE_INPUTS: &[InputContract] = &[];

const fn native_byte_nearest_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
            &[Architecture::Sse2],
            StrategyGuard::ResizeByteNearestVector { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::ResizeByteNearestVector { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::ResizeByteNearestVector { channels },
        ),
    ]
}

const fn native_byte_filtered_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
            &[Architecture::Sse2],
            StrategyGuard::ResizeByteNativeVector { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::ResizeByteNativeVector { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::ResizeByteNativeVector { channels },
        ),
    ]
}

const fn boxed_byte_mode_architectures(
    mode: Mode,
    channels: u8,
    nearest_only: bool,
) -> [ImplementationSupport; 3] {
    let straight_alpha = matches!(mode, Mode::ImageLumaA8 | Mode::ImageRgba8);
    let guard = if nearest_only {
        StrategyGuard::ResizeByteBoxedNearestVector { channels }
    } else {
        StrategyGuard::ResizeByteBoxedVector {
            channels,
            straight_alpha,
        }
    };
    [
        ImplementationSupport::cpu(
            OperationId::ResizeBoxed,
            mode,
            Architecture::Sse2,
            cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
            &[Architecture::Sse2],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::ResizeBoxed,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::ResizeBoxed,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
    ]
}

static RESIZE_ONE_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_nearest_architectures(Mode::One, 1);
static RESIZE_LUMA8_ARCHITECTURES: [ImplementationSupport; 3] = [
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageLuma8,
        Architecture::Sse2,
        cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
        &[Architecture::Sse2],
        StrategyGuard::ResizeByteNativeVector { channels: 1 },
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageLuma8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ResizeByteNativeVector { channels: 1 },
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageLuma8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::ResizeByteNativeVector { channels: 1 },
    ),
];
static RESIZE_LUMA_A8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageLumaA8, 2);
static RESIZE_LA_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageLaPremultiplied8, 2);
static RESIZE_PALETTE8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_nearest_architectures(Mode::ImagePalette8, 1);
static RESIZE_PALETTE_ALPHA8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImagePaletteAlpha8, 2);
static RESIZE_RGB8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageRgb8, 3);
static RESIZE_RGBA8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageRgba8, 4);
static RESIZE_RGB_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageRgbPremultiplied8, 4);
static RESIZE_RGBX8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageRgbx8, 4);
static RESIZE_CMYK8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageCmyk8, 4);
static RESIZE_YCBCR8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageYCbCr8, 3);
static RESIZE_LAB8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageLab8, 3);
static RESIZE_HSV8_ARCHITECTURES: [ImplementationSupport; 3] =
    native_byte_filtered_architectures(Mode::ImageHsv8, 3);

static RESIZE_BOXED_ONE_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::One, 1, true);
static RESIZE_BOXED_LUMA8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageLuma8, 1, false);
static RESIZE_BOXED_LUMA_A8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageLumaA8, 2, false);
static RESIZE_BOXED_LA_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageLaPremultiplied8, 2, false);
static RESIZE_BOXED_PALETTE8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImagePalette8, 1, true);
static RESIZE_BOXED_PALETTE_ALPHA8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImagePaletteAlpha8, 2, false);
static RESIZE_BOXED_RGB8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageRgb8, 3, false);
static RESIZE_BOXED_RGBA8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageRgba8, 4, false);
static RESIZE_BOXED_RGB_PREMULTIPLIED8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageRgbPremultiplied8, 4, false);
static RESIZE_BOXED_RGBX8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageRgbx8, 4, false);
static RESIZE_BOXED_CMYK8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageCmyk8, 4, false);
static RESIZE_BOXED_YCBCR8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageYCbCr8, 3, false);
static RESIZE_BOXED_LAB8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageLab8, 3, false);
static RESIZE_BOXED_HSV8_ARCHITECTURES: [ImplementationSupport; 3] =
    boxed_byte_mode_architectures(Mode::ImageHsv8, 3, false);

fn native_byte_architectures(mode: Mode) -> Option<&'static [ImplementationSupport]> {
    match mode {
        Mode::ImageLuma8 => Some(&RESIZE_LUMA8_ARCHITECTURES),
        Mode::One => Some(&RESIZE_ONE_ARCHITECTURES),
        Mode::ImageLumaA8 => Some(&RESIZE_LUMA_A8_ARCHITECTURES),
        Mode::ImageLaPremultiplied8 => Some(&RESIZE_LA_PREMULTIPLIED8_ARCHITECTURES),
        Mode::ImagePalette8 => Some(&RESIZE_PALETTE8_ARCHITECTURES),
        Mode::ImagePaletteAlpha8 => Some(&RESIZE_PALETTE_ALPHA8_ARCHITECTURES),
        Mode::ImageRgb8 => Some(&RESIZE_RGB8_ARCHITECTURES),
        Mode::ImageRgba8 => Some(&RESIZE_RGBA8_ARCHITECTURES),
        Mode::ImageRgbPremultiplied8 => Some(&RESIZE_RGB_PREMULTIPLIED8_ARCHITECTURES),
        Mode::ImageRgbx8 => Some(&RESIZE_RGBX8_ARCHITECTURES),
        Mode::ImageCmyk8 => Some(&RESIZE_CMYK8_ARCHITECTURES),
        Mode::ImageYCbCr8 => Some(&RESIZE_YCBCR8_ARCHITECTURES),
        Mode::ImageLab8 => Some(&RESIZE_LAB8_ARCHITECTURES),
        Mode::ImageHsv8 => Some(&RESIZE_HSV8_ARCHITECTURES),
        _ => None,
    }
}

fn native_byte_boxed_architectures(mode: Mode) -> Option<&'static [ImplementationSupport]> {
    match mode {
        Mode::One => Some(&RESIZE_BOXED_ONE_ARCHITECTURES),
        Mode::ImageLuma8 => Some(&RESIZE_BOXED_LUMA8_ARCHITECTURES),
        Mode::ImageLumaA8 => Some(&RESIZE_BOXED_LUMA_A8_ARCHITECTURES),
        Mode::ImageLaPremultiplied8 => Some(&RESIZE_BOXED_LA_PREMULTIPLIED8_ARCHITECTURES),
        Mode::ImagePalette8 => Some(&RESIZE_BOXED_PALETTE8_ARCHITECTURES),
        Mode::ImagePaletteAlpha8 => Some(&RESIZE_BOXED_PALETTE_ALPHA8_ARCHITECTURES),
        Mode::ImageRgb8 => Some(&RESIZE_BOXED_RGB8_ARCHITECTURES),
        Mode::ImageRgba8 => Some(&RESIZE_BOXED_RGBA8_ARCHITECTURES),
        Mode::ImageRgbPremultiplied8 => Some(&RESIZE_BOXED_RGB_PREMULTIPLIED8_ARCHITECTURES),
        Mode::ImageRgbx8 => Some(&RESIZE_BOXED_RGBX8_ARCHITECTURES),
        Mode::ImageCmyk8 => Some(&RESIZE_BOXED_CMYK8_ARCHITECTURES),
        Mode::ImageYCbCr8 => Some(&RESIZE_BOXED_YCBCR8_ARCHITECTURES),
        Mode::ImageLab8 => Some(&RESIZE_BOXED_LAB8_ARCHITECTURES),
        Mode::ImageHsv8 => Some(&RESIZE_BOXED_HSV8_ARCHITECTURES),
        _ => None,
    }
}

const RESIZE_I32_ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageI32,
        Architecture::AvxFma,
        cfg!(target_arch = "x86_64"),
        &[Architecture::AvxFma],
        StrategyGuard::ResizeFilteredFma,
    ),
    ImplementationSupport::cpu(
        OperationId::Resize,
        Mode::ImageI32,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::ResizeI32Neon,
    ),
];
const RESIZE_F32_ARCHITECTURES: &[ImplementationSupport] = &[ImplementationSupport::cpu(
    OperationId::Resize,
    Mode::ImageF32,
    Architecture::AvxFma,
    cfg!(target_arch = "x86_64"),
    &[Architecture::AvxFma],
    StrategyGuard::ResizeFilteredFma,
)];
const fn luma16_architectures(mode: Mode) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
            &[Architecture::Sse2],
            StrategyGuard::ResizeLuma16Vector,
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::ResizeLuma16Vector,
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::ResizeLuma16Vector,
        ),
    ]
}
static RESIZE_I16_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_architectures(Mode::ImageI16);
static RESIZE_I16L_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_architectures(Mode::ImageI16L);
static RESIZE_I16B_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_architectures(Mode::ImageI16B);
static RESIZE_I16N_ARCHITECTURES: [ImplementationSupport; 3] =
    luma16_architectures(Mode::ImageI16N);
const RESIZE_BOXED_I32_ARCHITECTURES: &[ImplementationSupport] = &[ImplementationSupport::cpu(
    OperationId::ResizeBoxed,
    Mode::ImageI32,
    Architecture::AvxFma,
    cfg!(target_arch = "x86_64"),
    &[Architecture::AvxFma],
    StrategyGuard::ResizeFilteredFma,
)];
const RESIZE_BOXED_F32_ARCHITECTURES: &[ImplementationSupport] = &[ImplementationSupport::cpu(
    OperationId::ResizeBoxed,
    Mode::ImageF32,
    Architecture::AvxFma,
    cfg!(target_arch = "x86_64"),
    &[Architecture::AvxFma],
    StrategyGuard::ResizeFilteredFma,
)];

fn supported_mode(operation: OperationId, mode: Mode) -> ModeSupport {
    if operation == OperationId::Resize
        && let Some(architectures) = native_byte_architectures(mode)
    {
        return ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            RESIZE_INPUTS,
            architectures,
        );
    }
    if operation == OperationId::ResizeBoxed
        && let Some(architectures) = native_byte_boxed_architectures(mode)
    {
        return ModeSupport::cpu(
            operation,
            mode,
            OutputModeRule::Preserve,
            RESIZE_INPUTS,
            architectures,
        );
    }
    let architectures = match (operation, mode) {
        (OperationId::Resize, Mode::ImageI32) => RESIZE_I32_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageF32) => RESIZE_F32_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageI16) => &RESIZE_I16_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageI16L) => &RESIZE_I16L_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageI16B) => &RESIZE_I16B_ARCHITECTURES,
        (OperationId::Resize, Mode::ImageI16N) => &RESIZE_I16N_ARCHITECTURES,
        (OperationId::ResizeBoxed, Mode::ImageI32) => RESIZE_BOXED_I32_ARCHITECTURES,
        (OperationId::ResizeBoxed, Mode::ImageF32) => RESIZE_BOXED_F32_ARCHITECTURES,
        _ => return ModeSupport::serial(operation, mode, RESIZE_INPUTS),
    };
    ModeSupport::cpu(operation, mode, OutputModeRule::Preserve, RESIZE_INPUTS, architectures)
}

fn classify_resize(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => supported_mode(OperationId::Resize, mode),
    }
}

fn classify_boxed_resize(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N
        | Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => supported_mode(OperationId::ResizeBoxed, mode),
    }
}

/// Exhaustive source-mode support for ordinary Resize.
pub(crate) static SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::Resize, RESIZE_INPUTS, classify_resize);

/// Exhaustive source-mode support for boxed Resize.
pub(crate) static BOXED_SUPPORT: OperationSupport = OperationSupport::new(
    OperationId::ResizeBoxed,
    RESIZE_INPUTS,
    classify_boxed_resize,
);

/// Resolve exact-mode filter policy before selecting or preparing a kernel.

#[path = "helpers/prepared.rs"]
mod prepared;
pub(crate) use prepared::{
    effective_filter, prepare_direct,
    prepare_direct_with_selection, prepare_pipeline, PreparedResize,
};
