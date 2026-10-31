//! Architecture declarations shared by the exact Paste mode strategies.

use crate::compute::Architecture;
use crate::compute::registry::support::{ImplementationSupport, OperationId, StrategyGuard};
use crate::raster::Mode;

/// Declares the 128-bit Paste implementations built for the target.
///
/// Each mode passes its exact storage width. The guard only admits a vector
/// route after its destination bytes match that mode's declared layout.
pub(crate) const fn cpu_architectures_for(
    mode: Mode,
    channels: u8,
) -> [ImplementationSupport; 3] {
    [
        ImplementationSupport::cpu(
            OperationId::Paste,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
            StrategyGuard::NativeBytes { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Paste,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::NativeBytes { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Paste,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::NativeBytes { channels },
        ),
    ]
}

/// Declares the generic architecture vector route and its eligible L-mask NEON subkernel.
///
/// The two NEON entries share one architecture identity: the first makes the
/// exact subkernel admission visible in support introspection, and the second
/// retains the generic vector fallback for every other valid Paste request.
pub(crate) const fn cpu_architectures_for_l_mask_neon(
    mode: Mode,
    channels: u8,
    minimum_clipped_width: u32,
) -> [ImplementationSupport; 4] {
    [
        ImplementationSupport::cpu(
            OperationId::Paste,
            mode,
            Architecture::Sse2,
            cfg!(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            )),
            &[Architecture::Sse2],
            StrategyGuard::NativeBytes { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Paste,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::PasteLMaskNeon {
                mode,
                channels,
                minimum_clipped_width,
            },
        ),
        ImplementationSupport::cpu(
            OperationId::Paste,
            mode,
            Architecture::Neon,
            cfg!(all(target_arch = "aarch64", target_feature = "neon")),
            &[Architecture::Neon],
            StrategyGuard::NativeBytes { channels },
        ),
        ImplementationSupport::cpu(
            OperationId::Paste,
            mode,
            Architecture::WasmSimd128,
            cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
            &[Architecture::WasmSimd128],
            StrategyGuard::NativeBytes { channels },
        ),
    ]
}

macro_rules! byte_architecture_impl {
    ($channels:literal, $image_type:path, $variant:ident) => {
        pub(crate) fn execute(
            strategy: $crate::compute::pool_cpu::ops::paste::helpers::plan::BytePlan<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            let (width, height) = strategy.destination_size;
            let bytes = $crate::compute::pool_cpu::ops::paste::helpers::vector::execute::<
                $channels,
            >(strategy)?;
            let image = <$image_type>::from_raw(width, height, bytes).ok_or_else(|| {
                $crate::error::PilError::InternalError(
                    "Paste vector output dimensions were invalid".into(),
                )
            })?;
            Ok($crate::raster::DynamicImage::$variant(image))
        }
    };
}

pub(crate) use byte_architecture_impl;

macro_rules! luma16_architecture_impl {
    ($variant:ident) => {
        pub(crate) fn execute(
            strategy: $crate::compute::pool_cpu::ops::paste::helpers::plan::Luma16Plan<'_>,
        ) -> Result<$crate::raster::DynamicImage, $crate::error::PilError> {
            let (width, height) = strategy.destination_size;
            let source_image = std::sync::Arc::clone(&strategy.source);
            let source = match source_image.as_ref() {
                $crate::raster::DynamicImage::$variant(image) => image.as_raw(),
                _ => {
                    return Err($crate::error::PilError::InternalError(
                        "I;16 vector Paste source mode changed after preparation".into(),
                    ));
                }
            };
            let samples = $crate::compute::pool_cpu::ops::paste::helpers::i16_vector::execute(
                strategy,
                source,
            )?;
            let image = $crate::raster::ImageBuffer::<
                $crate::raster::Luma<u16>,
                Vec<u16>,
            >::from_raw(width, height, samples)
            .ok_or_else(|| {
                $crate::error::PilError::InternalError(
                    "I;16 vector Paste output dimensions were invalid".into(),
                )
            })?;
            Ok($crate::raster::DynamicImage::$variant(image))
        }
    };
}

pub(crate) use luma16_architecture_impl;
