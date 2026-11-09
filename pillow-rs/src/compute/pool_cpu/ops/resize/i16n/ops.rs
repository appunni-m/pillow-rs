//! Exact ImageI16N strategy with Pillow's historical big-endian branch.

use crate::compute::registry::support::{ModeSupport, OperationId, StrategySelection};
use crate::raster::{DynamicImage, ImageBuffer, Luma, Mode};

static RESIZE_ARCHITECTURES: [crate::compute::registry::support::ImplementationSupport; 3] =
    super::super::helpers::support::luma16_architectures(Mode::ImageI16N);

pub(crate) const SUPPORT: ModeSupport =
    super::super::helpers::support::native_mode_support(Mode::ImageI16N, &RESIZE_ARCHITECTURES);
pub(crate) const BOXED_SUPPORT: ModeSupport = ModeSupport::serial(
    OperationId::ResizeBoxed,
    Mode::ImageI16N,
    super::super::helpers::support::INPUTS,
);

pub(crate) fn resize_selected(
    context: &crate::compute::ExecutionContext,
    selection: &StrategySelection,
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<DynamicImage, crate::error::PilError> {
    let architecture = selection.implementation.architecture;
    let output = if architecture == crate::compute::Architecture::Default {
        super::super::helpers::strategy::require_default_architecture(context, selection)?;
        super::default::resize_impl::execute(input, width, height, filter, bounds)
    } else {
        if bounds.is_some() {
            return Err(crate::error::PilError::InternalError(
                "ImageI16N vector Resize does not accept boxed bounds".into(),
            ));
        }
        let architecture =
            super::super::helpers::strategy::require_architecture(context, selection)?;
        match architecture {
            #[cfg(all(
                any(target_arch = "x86", target_arch = "x86_64"),
                target_feature = "sse2"
            ))]
            crate::compute::Architecture::Sse2 => {
                super::sse2::resize_impl::execute(input, width, height, filter)
            }
            #[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
            crate::compute::Architecture::Neon => {
                super::neon::resize_impl::execute(input, width, height, filter)
            }
            #[cfg(all(target_arch = "wasm32", target_feature = "simd128"))]
            crate::compute::Architecture::WasmSimd128 => {
                super::wasm_simd128::resize_impl::execute(input, width, height, filter)
            }
            _ => Err(crate::error::PilError::InternalError(format!(
                "ImageI16N Resize received unsupported architecture {architecture:?}"
            ))),
        }?
    };
    Ok(DynamicImage::ImageI16N(output))
}
