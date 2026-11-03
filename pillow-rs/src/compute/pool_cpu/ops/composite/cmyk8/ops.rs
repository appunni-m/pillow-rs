//! Exact-mode strategy entry for ImageCmyk8.

use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

pub(crate) const MODE: Mode = Mode::ImageCmyk8;

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
        |plan| crate::compute::pool_cpu::ops::paste::cmyk8::ops::execute(plan),
    )
}
