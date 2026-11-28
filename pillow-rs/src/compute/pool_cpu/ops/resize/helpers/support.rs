//! Shared declaration constructors for Resize mode-owned support contracts.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
};
use crate::compute::{Architecture, registry::support::ImplementationSupport};
use crate::raster::Mode;

pub(in crate::compute::pool_cpu::ops::resize) const INPUTS: &[InputContract] = &[];

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::resize) const fn native_byte_architectures(
    mode: Mode,
    channels: u8,
    nearest_only: bool,
) -> [ImplementationSupport; 3] {
    let guard = if nearest_only {
        StrategyGuard::ResizeByteNearestVector { channels }
    } else {
        StrategyGuard::ResizeByteNativeVector { channels }
    };
    byte_architectures(OperationId::Resize, mode, guard)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::resize) const fn native_byte_avx512f_architectures(
    mode: Mode,
) -> [ImplementationSupport; 4] {
    let guard = StrategyGuard::ResizeAvx512Nearest;
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Avx512F,
            cfg!(any(target_arch = "x86", target_arch = "x86_64")),
            &[Architecture::Avx512F],
            guard,
        ),
        byte_architecture(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            StrategyGuard::ResizeByteNativeVector { channels: 4 },
        ),
        byte_architecture(
            OperationId::Resize,
            mode,
            Architecture::Neon,
            StrategyGuard::ResizeByteNativeVector { channels: 4 },
        ),
        byte_architecture(
            OperationId::Resize,
            mode,
            Architecture::WasmSimd128,
            StrategyGuard::ResizeByteNativeVector { channels: 4 },
        ),
    ]
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::resize) const fn boxed_byte_architectures(
    mode: Mode,
    channels: u8,
    nearest_only: bool,
) -> [ImplementationSupport; 3] {
    let guard = if nearest_only {
        StrategyGuard::ResizeByteBoxedNearestVector { channels }
    } else {
        StrategyGuard::ResizeByteBoxedVector {
            channels,
            straight_alpha: matches!(mode, Mode::ImageLumaA8 | Mode::ImageRgba8),
        }
    };
    byte_architectures(OperationId::ResizeBoxed, mode, guard)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
const fn byte_architectures(
    operation: OperationId,
    mode: Mode,
    guard: StrategyGuard,
) -> [ImplementationSupport; 3] {
    [
        byte_architecture(operation, mode, Architecture::Sse2, guard),
        byte_architecture(operation, mode, Architecture::Neon, guard),
        byte_architecture(operation, mode, Architecture::WasmSimd128, guard),
    ]
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
const fn byte_architecture(
    operation: OperationId,
    mode: Mode,
    architecture: Architecture,
    guard: StrategyGuard,
) -> ImplementationSupport {
    let (supported, fallbacks): (bool, &'static [Architecture]) = match architecture {
        Architecture::Sse2 => (
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
        ),
        Architecture::Neon => (
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
        ),
        Architecture::WasmSimd128 => (
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
        ),
        _ => (false, &[]),
    };
    ImplementationSupport::cpu(operation, mode, architecture, supported, fallbacks, guard)
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::resize) const fn luma16_architectures(
    mode: Mode,
) -> [ImplementationSupport; 5] {
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::AvxFma,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx",
                target_feature = "fma"
            )),
            &[Architecture::AvxFma],
            StrategyGuard::ResizeLuma16Vector,
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Avx,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "avx",
                not(target_feature = "fma")
            )),
            &[Architecture::Avx],
            StrategyGuard::ResizeLuma16Vector,
        ),
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2",
                not(target_feature = "avx"),
                not(target_feature = "fma")
            )),
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::resize) const fn native_mode_support(
    mode: Mode,
    architectures: &'static [ImplementationSupport],
) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::Resize,
        mode,
        OutputModeRule::Preserve,
        INPUTS,
        architectures,
    )
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::resize) const fn boxed_mode_support(
    mode: Mode,
    architectures: &'static [ImplementationSupport],
) -> ModeSupport {
    ModeSupport::cpu(
        OperationId::ResizeBoxed,
        mode,
        OutputModeRule::Preserve,
        INPUTS,
        architectures,
    )
}
