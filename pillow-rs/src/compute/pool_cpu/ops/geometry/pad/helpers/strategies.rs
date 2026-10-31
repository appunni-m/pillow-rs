//! Architecture declarations shared by native-byte Pad modes.

use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, OperationId, StrategyGuard,
};
use crate::raster::Mode;

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
