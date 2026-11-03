//! Exact-mode RGBA AlphaComposite declaration and execution.

use std::sync::Arc;

use crate::compute::Architecture;
use crate::compute::ExecutionContext;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
    PaletteKnowledge, StrategyGuard, StrategyInputs, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbaImage};
use super::default;

pub(crate) const MODE: Mode = Mode::ImageRgba8;

const INPUTS: &[InputContract] = &[InputContract::SameModeSecondaryImage {
    mode: Mode::ImageRgba8,
    rejection_reason: "images do not match",
}];

const ARCHITECTURES: &[ImplementationSupport] = &[
    ImplementationSupport::cpu(
        OperationId::AlphaComposite,
        Mode::ImageRgba8,
        Architecture::Sse2,
        cfg!(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2")),
        &[Architecture::Sse2],
        StrategyGuard::AlphaCompositeVector { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::AlphaComposite,
        Mode::ImageRgba8,
        Architecture::Neon,
        cfg!(all(target_arch = "aarch64", target_feature = "neon")),
        &[Architecture::Neon],
        StrategyGuard::AlphaCompositeVector { channels: 4 },
    ),
    ImplementationSupport::cpu(
        OperationId::AlphaComposite,
        Mode::ImageRgba8,
        Architecture::WasmSimd128,
        cfg!(all(target_arch = "wasm32", target_feature = "simd128")),
        &[Architecture::WasmSimd128],
        StrategyGuard::AlphaCompositeVector { channels: 4 },
    ),
];

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::AlphaComposite,
    Mode::ImageRgba8,
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
    destination: &'a RgbaImage,
    source: Arc<DynamicImage>,
    architecture: ArchitectureStrategy,
    selection: StrategySelection,
}

/// Prepared full-frame AlphaComposite+Mirror execution for exact RGBA mode.
pub(crate) struct MirrorFusionStrategy<'a> {
    destination: &'a RgbaImage,
    source: Arc<DynamicImage>,
    architecture: ArchitectureStrategy,
    alpha_selection: StrategySelection,
    mirror_selection: StrategySelection,
}

/// Fused output and the work counts used by per-operation telemetry.
pub(crate) struct MirrorFusionExecution {
    pub(crate) output: DynamicImage,
    pub(crate) composite_vector_blocks: u64,
    pub(crate) composite_scalar_tail: u64,
    pub(crate) mirror_vector_blocks: u64,
    pub(crate) mirror_scalar_tail: u64,
}

impl Strategy<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.selection
    }

    pub(crate) fn execute(self) -> Result<DynamicImage, PilError> {
        let DynamicImage::ImageRgba8(source) = self.source.as_ref() else {
            return Err(PilError::InternalError(
                "RGBA AlphaComposite preparation changed its source mode".into(),
            ));
        };
        let output = match self.architecture {
            ArchitectureStrategy::Default => {
                super::default::alpha_composite_impl::execute(source, self.destination)?
            }
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
            ArchitectureStrategy::Sse2 => super::sse2::alpha_composite_impl::execute(source, self.destination)?,
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            ArchitectureStrategy::Neon => super::neon::alpha_composite_impl::execute(source, self.destination)?,
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            ArchitectureStrategy::WasmSimd128 => super::wasm_simd128::alpha_composite_impl::execute(source, self.destination)?,
        };
        Ok(DynamicImage::ImageRgba8(output))
    }
}

pub(crate) fn prepare<'a>(
    destination: &'a RgbaImage,
    source: Arc<DynamicImage>,
    selection: StrategySelection,
) -> Result<Strategy<'a>, PilError> {
    let architecture = bind_architecture(selection.implementation.architecture)?;
    Ok(Strategy { destination, source, architecture, selection })
}

fn bind_architecture(architecture: Architecture) -> Result<ArchitectureStrategy, PilError> {
    let strategy = match architecture {
        Architecture::Default => ArchitectureStrategy::Default,
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
        Architecture::Sse2 => ArchitectureStrategy::Sse2,
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon => ArchitectureStrategy::Neon,
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 => ArchitectureStrategy::WasmSimd128,
        _ => {
            return Err(PilError::InternalError(
                "unbound RGBA AlphaComposite architecture strategy".into(),
            ));
        }
    };
    Ok(strategy)
}

/// Selects one architecture for the fused implementation only when both
/// operations choose the same exact RGBA strategy.
pub(crate) fn prepare_mirror_fusion<'a>(
    destination: &'a RgbaImage,
    source: Arc<DynamicImage>,
    alpha_selection: StrategySelection,
    ctx: &ExecutionContext,
) -> Result<Option<MirrorFusionStrategy<'a>>, PilError> {
    let (width, height) = destination.dimensions();
    let DynamicImage::ImageRgba8(source_image) = source.as_ref() else {
        return Ok(None);
    };
    if source_image.dimensions() != destination.dimensions() {
        return Ok(None);
    }
    if width == 0 || height == 0 {
        return Ok(None);
    }
    let mirror_selection = select_mirror_strategy(destination, ctx)?;
    let architecture = alpha_selection.implementation.architecture;
    if architecture != mirror_selection.implementation.architecture
        || (architecture != Architecture::Default && width < 8)
    {
        return Ok(None);
    }
    let architecture = bind_architecture(architecture)?;
    Ok(Some(MirrorFusionStrategy {
        destination,
        source,
        architecture,
        alpha_selection,
        mirror_selection,
    }))
}

fn select_mirror_strategy(
    destination: &RgbaImage,
    ctx: &ExecutionContext,
) -> Result<StrategySelection, PilError> {
    let (width, height) = destination.dimensions();
    crate::compute::pool_cpu::ops::transpose::rgba8::ops::MIRROR_SUPPORT.select(
        ctx,
        StrategyInputs {
            width,
            height,
            byte_len: Some(destination.as_raw().len()),
            palette: PaletteKnowledge::Unknown,
            request: crate::compute::registry::support::RequestFacts::Unspecified,
        },
    )
}

impl MirrorFusionStrategy<'_> {
    pub(crate) fn selection(&self) -> &StrategySelection {
        &self.alpha_selection
    }

    pub(crate) fn mirror_selection(&self) -> &StrategySelection {
        &self.mirror_selection
    }

    pub(crate) fn execute(self) -> Result<MirrorFusionExecution, PilError> {
        let DynamicImage::ImageRgba8(source) = self.source.as_ref() else {
            return Err(PilError::InternalError(
                "RGBA AlphaComposite+Mirror preparation changed its source mode".into(),
            ));
        };
        let (width, height) = self.destination.dimensions();
        let (output, composite_vector_blocks, composite_scalar_tail,
            mirror_vector_blocks, mirror_scalar_tail) = match self.architecture {
            ArchitectureStrategy::Default => (
                super::default::alpha_composite_mirror_impl::execute(
                    source.as_raw(), self.destination.as_raw(), width, height,
                )?,
                0, 0, 0, 0,
            ),
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
            ArchitectureStrategy::Sse2 => fused_vector_output(source, self.destination)?,
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            ArchitectureStrategy::Neon => fused_vector_output(source, self.destination)?,
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            ArchitectureStrategy::WasmSimd128 => fused_vector_output(source, self.destination)?,
        };
        let output = RgbaImage::from_raw(width, height, output).ok_or_else(|| {
            PilError::InternalError("RGBA AlphaComposite+Mirror output shape mismatch".into())
        })?;
        Ok(MirrorFusionExecution {
            output: DynamicImage::ImageRgba8(output),
            composite_vector_blocks,
            composite_scalar_tail,
            mirror_vector_blocks,
            mirror_scalar_tail,
        })
    }
}

fn fused_vector_output(
    source: &RgbaImage,
    destination: &RgbaImage,
) -> Result<(Vec<u8>, u64, u64, u64, u64), PilError> {
    let (width, height) = destination.dimensions();
    super::helpers::alpha_composite_mirror::execute(
        source.as_raw(),
        destination.as_raw(),
        width,
        height,
    )
}

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::helpers::strategy::require_operation_mode(
        selection,
        OperationId::Merge,
        MODE,
    )?;
    match selection.implementation.architecture {
        Architecture::Default => super::default::operations::merge(source, remaining),
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
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
    super::super::helpers::blend::execute_selected(
        source, other, MODE, 4, alpha, selection,
    )
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
        4,
        selection,
        |plan| crate::compute::pool_cpu::ops::paste::rgba8::ops::execute(plan),
    )
}

