//! Architecture declarations shared by the exact I;16 crop identities.

use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, OperationId, StrategyGuard,
};
use crate::raster::Mode;

pub(crate) const fn luma16_copy_architectures(
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
