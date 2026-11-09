//! Architecture declarations shared by exact native-byte Chops modes.

use crate::compute::registry::support::StrategySelection;
use crate::compute::registry::support::{ImplementationSupport, OperationId, StrategyGuard};
use crate::compute::{Architecture, Backend};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) fn require_default_selection(
    selection: &StrategySelection,
    mode: Mode,
) -> Result<(), PilError> {
    let implementation = selection.implementation;
    if implementation.backend == Backend::Cpu
        && implementation.mode == mode
        && implementation.architecture == Architecture::Default
    {
        return Ok(());
    }
    Err(PilError::InternalError(
        "Chops Default mode received a mismatched strategy selection".into(),
    ))
}

pub(crate) const fn byte_architectures(
    operation: OperationId,
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    let guard = match operation {
        OperationId::ChopsAdd | OperationId::ChopsSubtract => {
            StrategyGuard::ChopsAddSubtractVector { channels }
        }
        OperationId::ChopsInvert => StrategyGuard::NativePointwiseBytesIncludingEmpty { channels },
        _ => StrategyGuard::ChopsEqualDimensions { channels },
    };
    architecture_strategies(operation, mode, guard)
}

pub(crate) const fn offset_byte_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    architecture_strategies(
        OperationId::ChopsOffset,
        mode,
        StrategyGuard::ChopsOffsetNativeBytes { channels },
    )
}

pub(crate) const fn offset_i16_architectures(mode: Mode) -> [ImplementationSupport; 3] {
    architecture_strategies(
        OperationId::ChopsOffset,
        mode,
        StrategyGuard::ChopsOffsetNativeI16,
    )
}

const fn architecture_strategies(
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

macro_rules! declare_byte_offset_architectures {
    ($mode:expr, $channels:expr) => {
        static OFFSET_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::offset_byte_architectures(
                $mode, $channels,
            );
    };
}

macro_rules! declare_i16_offset_architectures {
    ($mode:expr) => {
        static OFFSET_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::offset_i16_architectures(
                $mode,
            );
    };
}

macro_rules! declare_byte_invert_architectures {
    ($mode:expr, $channels:expr) => {
        static INVERT_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsInvert,
                $mode,
                $channels,
            );
    };
}

macro_rules! declare_byte_binary_architectures {
    ($mode:expr, $channels:expr) => {
        static ADD_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsAdd,
                $mode,
                $channels,
            );
        static SUBTRACT_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsSubtract,
                $mode,
                $channels,
            );
        static MULTIPLY_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsMultiply,
                $mode,
                $channels,
            );
        static SCREEN_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsScreen,
                $mode,
                $channels,
            );
        static DARKER_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsDarker,
                $mode,
                $channels,
            );
        static LIGHTER_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsLighter,
                $mode,
                $channels,
            );
        static DIFFERENCE_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsDifference,
                $mode,
                $channels,
            );
        static OVERLAY_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsOverlay,
                $mode,
                $channels,
            );
        static HARD_LIGHT_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsHardLight,
                $mode,
                $channels,
            );
        static SOFT_LIGHT_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsSoftLight,
                $mode,
                $channels,
            );
        static ADD_MODULO_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsAddModulo,
                $mode,
                $channels,
            );
        static SUBTRACT_MODULO_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsSubtractModulo,
                $mode,
                $channels,
            );
        static OFFSET_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::offset_byte_architectures(
                $mode, $channels,
            );
        fn architecture_for(operation: OperationId) -> &'static [ImplementationSupport] {
            match operation {
                OperationId::ChopsOffset => &OFFSET_ARCHITECTURES,
                OperationId::ChopsAdd => &ADD_ARCHITECTURES,
                OperationId::ChopsSubtract => &SUBTRACT_ARCHITECTURES,
                OperationId::ChopsMultiply => &MULTIPLY_ARCHITECTURES,
                OperationId::ChopsScreen => &SCREEN_ARCHITECTURES,
                OperationId::ChopsDarker => &DARKER_ARCHITECTURES,
                OperationId::ChopsLighter => &LIGHTER_ARCHITECTURES,
                OperationId::ChopsDifference => &DIFFERENCE_ARCHITECTURES,
                OperationId::ChopsOverlay => &OVERLAY_ARCHITECTURES,
                OperationId::ChopsHardLight => &HARD_LIGHT_ARCHITECTURES,
                OperationId::ChopsSoftLight => &SOFT_LIGHT_ARCHITECTURES,
                OperationId::ChopsAddModulo => &ADD_MODULO_ARCHITECTURES,
                OperationId::ChopsSubtractModulo => &SUBTRACT_MODULO_ARCHITECTURES,
                _ => &[],
            }
        }
    };
}

macro_rules! declare_logical_architectures {
    ($mode:expr) => {
        static LOGICAL_AND_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsLogicalAnd,
                $mode,
                1,
            );
        static LOGICAL_OR_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsLogicalOr,
                $mode,
                1,
            );
        static LOGICAL_XOR_ARCHITECTURES: [ImplementationSupport; 3] =
            $crate::compute::pool_cpu::ops::chops::helpers::strategy::byte_architectures(
                OperationId::ChopsLogicalXor,
                $mode,
                1,
            );
    };
}

pub(crate) use declare_byte_binary_architectures;
pub(crate) use declare_byte_invert_architectures;
pub(crate) use declare_byte_offset_architectures;
pub(crate) use declare_i16_offset_architectures;
pub(crate) use declare_logical_architectures;
