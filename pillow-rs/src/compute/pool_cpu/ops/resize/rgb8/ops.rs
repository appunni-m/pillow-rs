//! Exact rgb8 mode selection for Resize.

use crate::compute::{Architecture, Backend, ExecutionContext};
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
                "rgb8 Resize received unsupported architecture {architecture:?}"
            ))),
        },
        |bytes| {
            RgbImage::from_raw(width, height, bytes).ok_or_else(|| {
                PilError::ValueError("resize_rgb: failed to create output buffer".into())
            })
        },
    )
}

/// Executes sparse RGB work through the architecture retained by its prepared
/// Resize or ResizeBoxed selection.
pub(crate) fn resize_with_sparse_rows(
    context: &ExecutionContext,
    architecture: Architecture,
    image: &RgbImage,
    width: u32,
    height: u32,
    bounds: Option<(f64, f64, f64, f64)>,
    filter: ResampleFilter,
    sparse_source_rows: &[bool],
) -> Result<RgbImage, PilError> {
    if context.backend() != Backend::Cpu {
        return Err(PilError::InternalError(
            "sparse RGB Resize requires the CPU backend".into(),
        ));
    }
    let bytes = match (architecture, bounds) {
        (Architecture::Default, Some(bounds)) => {
            return super::default::resize_impl::execute_boxed_with_sparse_rows(
                image, width, height, bounds, filter, sparse_source_rows,
            );
        }
        (Architecture::Default, None) => {
            return super::default::resize_impl::execute(image, width, height, filter, None);
        }
        (Architecture::Sse2, Some(bounds)) => {
            super::sse2::resize_impl::execute_boxed_with_sparse_rows(
                image, width, height, bounds, filter, sparse_source_rows,
            )?
        }
        (Architecture::Sse2, None) => super::sse2::resize_impl::execute_with_sparse_rows(
            image, width, height, filter, sparse_source_rows,
        )?,
        (Architecture::Neon, Some(bounds)) => {
            super::neon::resize_impl::execute_boxed_with_sparse_rows(
                image, width, height, bounds, filter, sparse_source_rows,
            )?
        }
        (Architecture::Neon, None) => super::neon::resize_impl::execute_with_sparse_rows(
            image, width, height, filter, sparse_source_rows,
        )?,
        (Architecture::WasmSimd128, Some(bounds)) => {
            super::wasm_simd128::resize_impl::execute_boxed_with_sparse_rows(
                image, width, height, bounds, filter, sparse_source_rows,
            )?
        }
        (Architecture::WasmSimd128, None) => {
            super::wasm_simd128::resize_impl::execute_with_sparse_rows(
                image, width, height, filter, sparse_source_rows,
            )?
        }
        (architecture, _) => {
            return Err(PilError::InternalError(format!(
                "sparse RGB Resize received unsupported architecture {architecture:?}"
            )));
        }
    };
    RgbImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::ValueError("resize_rgb: failed to create output buffer".into())
    })
}
