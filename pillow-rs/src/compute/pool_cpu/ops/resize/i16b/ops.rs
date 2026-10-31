//! Exact ImageI16B strategy with Pillow's big-endian resampling contract.

use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, ImageBuffer, Luma};

pub(crate) fn resize(
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> DynamicImage {
    DynamicImage::ImageI16B(super::default::resize_impl::execute(
        input, width, height, filter, bounds,
    ))
}

pub(crate) fn resize_selected(
    context: &crate::compute::ExecutionContext,
    architecture: crate::compute::Architecture,
    input: &ImageBuffer<Luma<u16>, Vec<u16>>,
    width: u32,
    height: u32,
    filter: crate::pipeline::ResampleFilter,
    bounds: super::super::helpers::typed_nearest::BoxBounds,
) -> Result<DynamicImage, crate::error::PilError> {
    let output = if architecture == crate::compute::Architecture::Default {
        super::super::helpers::strategy::require_default_architecture(context, architecture)?;
        super::default::resize_impl::execute(input, width, height, filter, bounds)
    } else {
        if bounds.is_some() {
            return Err(crate::error::PilError::InternalError(
                "ImageI16B vector Resize does not accept boxed bounds".into(),
            ));
        }
        super::super::helpers::strategy::require_architecture(context, architecture)?;
        match architecture {
            #[cfg(all(any(target_arch = "x86", target_arch = "x86_64"), target_feature = "sse2"))]
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
                "ImageI16B Resize received unsupported architecture {architecture:?}"
            ))),
        }?
    };
    Ok(DynamicImage::ImageI16B(output))
}
