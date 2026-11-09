//! Exact-mode LA AlphaComposite declaration and execution.

use std::sync::Arc;

use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule, StrategyGuard,
    StrategySelection,
};
use crate::compute::{Architecture, Backend};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageLumaA8;

const INPUTS: &[InputContract] = &[InputContract::SameModeSecondaryImage {
    mode: Mode::ImageLumaA8,
    rejection_reason: "images do not match",
}];

const ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::AlphaComposite,
        Mode::ImageLumaA8,
        Architecture::Sse2,
        cfg!(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2"
        )),
        &[Architecture::Sse2],
        StrategyGuard::AlphaCompositeVector { channels: 2 },
    ),
    ImplementationSupport::cpu(
        OperationId::AlphaComposite,
        Mode::ImageLumaA8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::AlphaCompositeVector { channels: 2 },
    ),
    ImplementationSupport::cpu(
        OperationId::AlphaComposite,
        Mode::ImageLumaA8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::AlphaCompositeVector { channels: 2 },
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::AlphaComposite,
    Mode::ImageLumaA8,
    OutputModeRule::Preserve,
    INPUTS,
    ARCHITECTURES,
);

#[derive(Clone, Copy)]
enum ArchitectureStrategy {
    Default,
    #[cfg(all(
        any(target_arch = "x86", target_arch = "x86_64"),
        target_feature = "sse2"
    ))]
    Sse2,
    #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
    Neon,
    #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
    WasmSimd128,
}

pub(crate) struct Strategy<'a> {
    destination: &'a GrayAlphaImage,
    source: Arc<DynamicImage>,
    architecture: ArchitectureStrategy,
    selection: StrategySelection,
}

impl Strategy<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let DynamicImage::ImageLumaA8(source) = self.source.as_ref() else {
            return Err(PilError::InternalError(
                "LA AlphaComposite preparation changed its source mode".into(),
            ));
        };
        let mut output = match self.architecture {
            ArchitectureStrategy::Default => {
                super::default::alpha_composite_impl::execute(source, self.destination)?
            }
            #[cfg(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            ))]
            ArchitectureStrategy::Sse2 => {
                super::sse2::alpha_composite_impl::execute(source, self.destination)?
            }
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            ArchitectureStrategy::Neon => {
                super::neon::alpha_composite_impl::execute(source, self.destination)?
            }
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            ArchitectureStrategy::WasmSimd128 => {
                super::wasm_simd128::alpha_composite_impl::execute(source, self.destination)?
            }
        };
        super::super::helpers::alpha_composite::preserve_luma_alpha_lanes(
            source,
            self.destination,
            &mut output,
        )?;
        Ok(DynamicImage::ImageLumaA8(output))
    }
}

pub(crate) fn prepare<'a>(
    destination: &'a GrayAlphaImage,
    source: Arc<DynamicImage>,
    selection: StrategySelection,
) -> Result<Strategy<'a>, PilError> {
    super::super::helpers::strategy::require_operation_mode(
        &selection,
        OperationId::AlphaComposite,
        MODE,
    )?;
    if selection.implementation.backend != Backend::Cpu
        || selection.requested_backend != Backend::Cpu
    {
        return Err(PilError::InternalError(
            "LA AlphaComposite received a non-CPU strategy selection".into(),
        ));
    }
    let architecture = match selection.implementation.architecture {
        Architecture::Default => ArchitectureStrategy::Default,
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2"
        ))]
        Architecture::Sse2 => ArchitectureStrategy::Sse2,
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => ArchitectureStrategy::Neon,
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => ArchitectureStrategy::WasmSimd128,
        _ => {
            return Err(PilError::InternalError(
                "unbound LA AlphaComposite architecture strategy".into(),
            ));
        }
    };
    Ok(Strategy {
        destination,
        source,
        architecture,
        selection,
    })
}

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::strategy::require_operation_mode(selection, OperationId::Merge, MODE)?;
    match selection.implementation.architecture {
        Architecture::Default => super::default::operations::merge(source, remaining),
        #[cfg(all(
            any(target_arch = "x86", target_arch = "x86_64"),
            target_feature = "sse2"
        ))]
        Architecture::Sse2 => super::sse2::merge_impl::merge(source, remaining),
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => super::neon::merge_impl::merge(source, remaining),
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => super::wasm_simd128::merge_impl::merge(source, remaining),
        architecture => Err(PilError::InternalError(format!(
            "unbound {architecture:?} Merge strategy for mode {MODE:?}"
        ))),
    }
}

pub(crate) fn blend(
    source: &DynamicImage,
    other: &DynamicImage,
    alpha: f64,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::blend::execute_selected(source, other, MODE, 2, alpha, selection)
}

pub(crate) fn composite(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::composite::execute_byte_selected(
        source,
        destination,
        mask,
        mask_alpha,
        MODE,
        2,
        selection,
        |plan| crate::compute::pool_cpu::ops::paste::luma_a8::ops::execute(plan),
    )
}
