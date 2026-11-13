//! Shared declaration constructors for Resize mode-owned support contracts.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
};
use crate::compute::{Architecture, registry::support::ImplementationSupport};
use crate::raster::Mode;

pub(in crate::compute::pool_cpu::ops::resize) const INPUTS: &[InputContract] = &[];

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
const fn byte_architectures(
    operation: OperationId,
    mode: Mode,
    guard: StrategyGuard,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            operation,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
    ]
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(in crate::compute::pool_cpu::ops::resize) const fn luma16_architectures(
    mode: Mode,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Resize,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
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
