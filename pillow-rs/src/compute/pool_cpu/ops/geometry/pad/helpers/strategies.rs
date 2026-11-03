//! Architecture declarations shared by native-byte Pad modes.

use crate::compute::{Architecture, Backend};
use crate::compute::registry::support::{
    ImplementationSupport, OperationId, StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) fn selected_architecture(
    selection: &StrategySelection,
    mode: Mode,
) -> Result<Architecture, PilError> {
    let implementation = selection.implementation;
    if implementation.operation == OperationId::Pad
        && implementation.mode == mode
        && implementation.backend == Backend::Cpu
    {
        return Ok(implementation.architecture);
    }
    Err(PilError::InternalError(
        "Pad mode received a mismatched CPU strategy selection".into(),
    ))
}

pub(crate) fn require_default_selection(
    selection: &StrategySelection,
    mode: Mode,
) -> Result<(), PilError> {
    if selected_architecture(selection, mode)? == Architecture::Default {
        return Ok(());
    }
    Err(PilError::InternalError(
        "Pad Default mode received a non-Default strategy selection".into(),
    ))
}

pub(crate) const fn byte_vector_architectures(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    let guard = StrategyGuard::NativeBytes { channels };
    [
        ImplementationSupport::cpu(
            OperationId::Pad,
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
            OperationId::Pad,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            guard,
        ),
        ImplementationSupport::cpu(
            OperationId::Pad,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            guard,
        ),
    ]
}
