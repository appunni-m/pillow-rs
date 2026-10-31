//! Composite dispatch and validated mask sample preparation.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use crate::compute::Architecture;
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::compute::pool_cpu::ops::composite::ops::CompositeLayout;

pub(crate) fn execute_selected(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
    mode: Mode,
    layout: CompositeLayout,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    super::super::strategy::require_operation_mode(
        selection,
        OperationId::CompositeModule,
        mode,
    )?;
    match selection.implementation.architecture {
        Architecture::Default => execute(source, destination, mask, mask_alpha, mode, layout),
        #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
        Architecture::Sse2 if vector_mode(mode) => match layout {
            CompositeLayout::Byte { channels } => {
                super::vector::execute(source, destination, mask, mask_alpha, mode, channels)
            }
            _ => Err(unbound(mode, selection)),
        },
        #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
        Architecture::Neon if vector_mode(mode) => match layout {
            CompositeLayout::Byte { channels } => {
                super::vector::execute(source, destination, mask, mask_alpha, mode, channels)
            }
            _ => Err(unbound(mode, selection)),
        },
        #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
        Architecture::WasmSimd128 if vector_mode(mode) => match layout {
            CompositeLayout::Byte { channels } => {
                super::vector::execute(source, destination, mask, mask_alpha, mode, channels)
            }
            _ => Err(unbound(mode, selection)),
        },
        _ => Err(unbound(mode, selection)),
    }
}

fn vector_mode(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::One
            | Mode::ImageLuma8
            | Mode::ImagePalette8
            | Mode::ImageLumaA8
            | Mode::ImagePaletteAlpha8
            | Mode::ImageRgb8
            | Mode::ImageHsv8
            | Mode::ImageYCbCr8
            | Mode::ImageRgba8
            | Mode::ImageCmyk8
            | Mode::ImageRgbx8
    )
}

fn unbound(mode: Mode, selection: &StrategySelection) -> PilError {
    PilError::InternalError(format!(
        "unbound {:?} Composite strategy for mode {mode:?}",
        selection.implementation.architecture
    ))
}

pub(crate) fn execute(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: &DynamicImage,
    mask_alpha: bool,
    mode: Mode,
    layout: CompositeLayout,
) -> Result<DynamicImage, PilError> {
    let samples = super::mask::samples(mask, mask_alpha)?;
    match layout {
        CompositeLayout::Byte { channels } => {
            super::bytes::execute(source, destination, samples, mode, channels)
        }
        CompositeLayout::U16 { channels } => {
            super::numeric::execute_u16(source, destination, samples, mode, channels)
        }
        CompositeLayout::I32 => super::numeric::execute_i32(source, destination, samples),
        CompositeLayout::F32 { channels } => {
            super::numeric::execute_f32(source, destination, samples, mode, channels)
        }
    }
}
