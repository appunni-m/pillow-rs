//! Exact-mode LA AlphaComposite declaration and execution.

use std::sync::Arc;

use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    StrategyGuard, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode};
use super::default;

const INPUTS: &[InputContract] = &[InputContract::SameModeSecondaryImage {
    mode: Mode::ImageLumaA8,
    rejection_reason: "images do not match",
}];

const ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::AlphaComposite,
        Mode::ImageLumaA8,
        Architecture::Sse2,
        cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
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
    #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
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
        let output = match self.architecture {
            ArchitectureStrategy::Default => {
                default::alpha_composite_impl::execute(source, self.destination)?
            }
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
            ArchitectureStrategy::Sse2 => execute_vector(source, self.destination)?,
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            ArchitectureStrategy::Neon => execute_vector(source, self.destination)?,
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            ArchitectureStrategy::WasmSimd128 => execute_vector(source, self.destination)?,
        };
        Ok(DynamicImage::ImageLumaA8(output))
    }
}

pub(crate) fn prepare<'a>(
    destination: &'a GrayAlphaImage,
    source: Arc<DynamicImage>,
    selection: StrategySelection,
) -> Result<Strategy<'a>, PilError> {
    let architecture = match selection.implementation.architecture {
        Architecture::Default => ArchitectureStrategy::Default,
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
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
    Ok(Strategy { destination, source, architecture, selection })
}

fn execute_vector(
    source: &GrayAlphaImage,
    destination: &GrayAlphaImage,
) -> Result<GrayAlphaImage, PilError> {
    let (width, height) = destination.dimensions();
    let output = crate::compute::pool_cpu::ops::composite::helpers::alpha_composite_vector::apply::<2>(
        source.as_raw(),
        destination.as_raw(),
        width,
        height,
        "LA",
    )?;
    GrayAlphaImage::from_raw(width, height, output)
        .ok_or_else(|| PilError::InternalError("LA AlphaComposite output shape mismatch".into()))
}

