//! Exact luma_a8 mode selection for Resize.

use crate::compute::{Architecture, ExecutionContext};
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::GrayAlphaImage;

pub(crate) fn resize(
    context: &ExecutionContext,
    selection: &StrategySelection,
    image: &GrayAlphaImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<GrayAlphaImage, PilError> {
    let architecture = selection.implementation.architecture;
    super::super::helpers::strategy::resize_byte_native::<_, 2>(
        context,
        selection,
        image.as_raw(),
        image.dimensions(),
        width,
        height,
        filter,
        bounds,
        || super::default::resize_impl::execute(image, width, height, filter, bounds),
        |bounds| match architecture {
            Architecture::Sse2 => match bounds {
                Some(bounds) => super::sse2::resize_impl::execute_boxed(
                    image, width, height, bounds, filter,
                ),
                None => super::sse2::resize_impl::execute(image, width, height, filter),
            },
            Architecture::Neon => match bounds {
                Some(bounds) => super::neon::resize_impl::execute_boxed(
                    image, width, height, bounds, filter,
                ),
                None => super::neon::resize_impl::execute(image, width, height, filter),
            },
            Architecture::WasmSimd128 => match bounds {
                Some(bounds) => super::wasm_simd128::resize_impl::execute_boxed(
                    image, width, height, bounds, filter,
                ),
                None => super::wasm_simd128::resize_impl::execute(image, width, height, filter),
            },
            _ => Err(PilError::InternalError(format!(
                "luma_a8 Resize received unsupported architecture {architecture:?}"
            ))),
        },
        |bytes| {
            GrayAlphaImage::from_raw(width, height, bytes).ok_or_else(|| {
                PilError::ValueError("resize_la: failed to create output buffer".into())
            })
        },
    )
}
