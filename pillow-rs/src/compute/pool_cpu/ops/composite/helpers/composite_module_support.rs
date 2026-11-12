//! Shared input contract and compile-time strategy declarations for CompositeModule modes.

use crate::compute::Architecture;
use crate::compute::registry::support::InputContract;
use crate::error::PilError;
use crate::raster::Mode;

pub(crate) const COMPOSITE_LUMA_MASKS: &[Mode] = &[Mode::One, Mode::ImageLuma8];
pub(crate) const COMPOSITE_ALPHA_MASKS: &[Mode] = &[
    Mode::ImageLumaA8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
];
pub(crate) const COMPOSITE_MODULE_INPUT: InputContract = InputContract::CompositeInputs {
    luma_masks: COMPOSITE_LUMA_MASKS,
    alpha_masks: COMPOSITE_ALPHA_MASKS,
    mode_rejection: "images do not match",
    mask_rejection: "bad transparency mask",
    size_rejection: "images do not match",
};
pub(crate) const COMPOSITE_MODULE_INPUTS: &[InputContract] = &[COMPOSITE_MODULE_INPUT];

pub(crate) const COMPOSITE_SSE2: &[Architecture] = &[Architecture::Sse2];
pub(crate) const COMPOSITE_NEON: &[Architecture] = &[Architecture::Neon];
pub(crate) const COMPOSITE_WASM_SIMD128: &[Architecture] = &[Architecture::WasmSimd128];

macro_rules! declare_composite_module_support {
    ($mode:ident, vector($channels:literal)) => {
        pub(crate) const COMPOSITE_MODULE_MODE_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::CompositeModule,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_MODULE_INPUTS,
                &[
                    crate::compute::registry::support::ImplementationSupport::cpu(
                        crate::compute::registry::support::OperationId::CompositeModule,
                        crate::raster::Mode::$mode,
                        crate::compute::Architecture::Sse2,
                        cfg!(all(
                            any(target_arch = "x86", target_arch = "x86_64"),
                            target_feature = "sse2"
                        )),
                        crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_SSE2,
                        crate::compute::registry::support::StrategyGuard::CompositeNativeByteVector {
                            mode: crate::raster::Mode::$mode,
                            channels: $channels,
                        },
                    ),
                    crate::compute::registry::support::ImplementationSupport::cpu(
                        crate::compute::registry::support::OperationId::CompositeModule,
                        crate::raster::Mode::$mode,
                        crate::compute::Architecture::Neon,
                        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                        crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_NEON,
                        crate::compute::registry::support::StrategyGuard::CompositeNativeByteVector {
                            mode: crate::raster::Mode::$mode,
                            channels: $channels,
                        },
                    ),
                    crate::compute::registry::support::ImplementationSupport::cpu(
                        crate::compute::registry::support::OperationId::CompositeModule,
                        crate::raster::Mode::$mode,
                        crate::compute::Architecture::WasmSimd128,
                        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                        crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_WASM_SIMD128,
                        crate::compute::registry::support::StrategyGuard::CompositeNativeByteVector {
                            mode: crate::raster::Mode::$mode,
                            channels: $channels,
                        },
                    ),
                ],
            );
    };
    ($mode:ident, luma_mask($channels:literal, $minimum_width:literal)) => {
        pub(crate) const COMPOSITE_MODULE_MODE_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::CompositeModule,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_MODULE_INPUTS,
                &[
                    crate::compute::registry::support::ImplementationSupport::cpu(
                        crate::compute::registry::support::OperationId::CompositeModule,
                        crate::raster::Mode::$mode,
                        crate::compute::Architecture::Sse2,
                        cfg!(all(
                            any(target_arch = "x86", target_arch = "x86_64"),
                            target_feature = "sse2"
                        )),
                        crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_SSE2,
                        crate::compute::registry::support::StrategyGuard::CompositeNativeByteVector {
                            mode: crate::raster::Mode::$mode,
                            channels: $channels,
                        },
                    ),
                    crate::compute::registry::support::ImplementationSupport::cpu(
                        crate::compute::registry::support::OperationId::CompositeModule,
                        crate::raster::Mode::$mode,
                        crate::compute::Architecture::Neon,
                        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                        crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_NEON,
                        crate::compute::registry::support::StrategyGuard::CompositeLMaskNeon {
                            mode: crate::raster::Mode::$mode,
                            channels: $channels,
                            minimum_clipped_width: $minimum_width,
                        },
                    ),
                    crate::compute::registry::support::ImplementationSupport::cpu(
                        crate::compute::registry::support::OperationId::CompositeModule,
                        crate::raster::Mode::$mode,
                        crate::compute::Architecture::Neon,
                        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
                        crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_NEON,
                        crate::compute::registry::support::StrategyGuard::CompositeNativeByteVector {
                            mode: crate::raster::Mode::$mode,
                            channels: $channels,
                        },
                    ),
                    crate::compute::registry::support::ImplementationSupport::cpu(
                        crate::compute::registry::support::OperationId::CompositeModule,
                        crate::raster::Mode::$mode,
                        crate::compute::Architecture::WasmSimd128,
                        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
                        crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_WASM_SIMD128,
                        crate::compute::registry::support::StrategyGuard::CompositeNativeByteVector {
                            mode: crate::raster::Mode::$mode,
                            channels: $channels,
                        },
                    ),
                ],
            );
    };
    ($mode:ident, scalar) => {
        pub(crate) const COMPOSITE_MODULE_MODE_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu(
                crate::compute::registry::support::OperationId::CompositeModule,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_MODULE_INPUTS,
                &[],
            );
    };
    ($mode:ident, core_only) => {
        pub(crate) const COMPOSITE_MODULE_MODE_SUPPORT: crate::compute::registry::support::ModeSupport =
            crate::compute::registry::support::ModeSupport::cpu_core_only(
                crate::compute::registry::support::OperationId::CompositeModule,
                crate::raster::Mode::$mode,
                crate::compute::registry::support::OutputModeRule::Preserve,
                crate::compute::pool_cpu::ops::composite::helpers::composite_module_support::COMPOSITE_MODULE_INPUTS,
                &[],
                "this exact high-depth raster identity is a core extension",
            );
    };
}

pub(crate) use declare_composite_module_support;

/// Validates Composite's eager wrapper size check from its declared contract.
pub(crate) fn validate_composite_wrapper_dimensions(
    source_size: (u32, u32),
    mask_size: (u32, u32),
) -> Result<(), PilError> {
    let InputContract::CompositeInputs { size_rejection, .. } = COMPOSITE_MODULE_INPUT else {
        return Err(PilError::InternalError(
            "Composite support omitted its input contract".into(),
        ));
    };
    if source_size != mask_size {
        return Err(PilError::ValueError(size_rejection.into()));
    }
    Ok(())
}

/// Selects luma or alpha mask samples from Composite's declared mode sets.
pub(crate) fn composite_wrapper_mask_alpha(mask_mode: Mode) -> Result<bool, PilError> {
    let InputContract::CompositeInputs {
        luma_masks,
        alpha_masks,
        mask_rejection,
        ..
    } = COMPOSITE_MODULE_INPUT
    else {
        return Err(PilError::InternalError(
            "Composite support omitted its mask contract".into(),
        ));
    };
    if luma_masks.contains(&mask_mode) {
        return Ok(false);
    }
    if alpha_masks.contains(&mask_mode) {
        return Ok(true);
    }
    Err(PilError::ValueError(mask_rejection.into()))
}
