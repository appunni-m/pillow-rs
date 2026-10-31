//! Exact hsv8 mode selection for Resize.

use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::RgbImage;

pub(crate) fn resize(
    context: &ExecutionContext,
    architecture: Architecture,
    image: &RgbImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbImage, PilError> {
    super::super::helpers::strategy::resize_byte_native::<_, 3>(
        context,
        architecture,
        image.as_raw(),
        image.dimensions(),
        width,
        height,
        filter,
        bounds,
        || super::default::resize_impl::execute(image, width, height, filter, bounds),
        |architecture, bounds| match architecture {
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
                "hsv8 Resize received unsupported architecture {architecture:?}"
            ))),
        },
        |bytes| {
            RgbImage::from_raw(width, height, bytes).ok_or_else(|| {
                PilError::ValueError("resize_hsv: failed to create output buffer".into())
            })
        },
    )
}
