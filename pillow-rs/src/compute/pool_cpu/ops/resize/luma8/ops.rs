//! Exact ImageLuma8 mode dispatch for Resize.

use crate::compute::registry::support::{ImplementationSupport, ModeSupport, StrategySelection};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{GrayImage, Mode};

static RESIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::support::native_byte_architectures(Mode::ImageLuma8, 1, false);
static RESIZE_BOXED_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::support::boxed_byte_architectures(Mode::ImageLuma8, 1, false);

pub(crate) const SUPPORT: ModeSupport =
    super::super::helpers::support::native_mode_support(Mode::ImageLuma8, &RESIZE_ARCHITECTURES);
pub(crate) const BOXED_SUPPORT: ModeSupport = super::super::helpers::support::boxed_mode_support(
    Mode::ImageLuma8,
    &RESIZE_BOXED_ARCHITECTURES,
);

pub(crate) fn resize(
    context: &ExecutionContext,
    selection: &StrategySelection,
    image: &GrayImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<GrayImage, PilError> {
    let architecture = selection.implementation.architecture;
    if !matches!(filter, ResampleFilter::Nearest) {
        match architecture {
            Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128 => {
                let architecture =
                    super::super::helpers::strategy::require_architecture(context, selection)?;
                return match architecture {
                    Architecture::Sse2 => match bounds {
                        Some(bounds) => GrayImage::from_raw(
                            width,
                            height,
                            super::sse2::resize_impl::execute_boxed(
                                image, width, height, bounds, filter,
                            )?,
                        )
                        .ok_or_else(|| {
                            PilError::ValueError("resize_l: failed to create output buffer".into())
                        }),
                        None => super::sse2::resize_impl::execute(image, width, height, filter),
                    },
                    Architecture::Neon => match bounds {
                        Some(bounds) => GrayImage::from_raw(
                            width,
                            height,
                            super::neon::resize_impl::execute_boxed(
                                image, width, height, bounds, filter,
                            )?,
                        )
                        .ok_or_else(|| {
                            PilError::ValueError("resize_l: failed to create output buffer".into())
                        }),
                        None => super::neon::resize_impl::execute(image, width, height, filter),
                    },
                    Architecture::WasmSimd128 => match bounds {
                        Some(bounds) => GrayImage::from_raw(
                            width,
                            height,
                            super::wasm_simd128::resize_impl::execute_boxed(
                                image, width, height, bounds, filter,
                            )?,
                        )
                        .ok_or_else(|| {
                            PilError::ValueError("resize_l: failed to create output buffer".into())
                        }),
                        None => {
                            super::wasm_simd128::resize_impl::execute(image, width, height, filter)
                        }
                    },
                    _ => unreachable!(),
                };
            }
            Architecture::Default => (),
            _ => {
                return Err(PilError::InternalError(format!(
                    "ImageLuma8 Resize received unsupported architecture {architecture:?}"
                )));
            }
        }
    }
    super::super::helpers::strategy::resize_byte_nearest::<_, 1>(
        context,
        selection,
        image.as_raw(),
        image.dimensions(),
        width,
        height,
        filter,
        bounds,
        || super::default::resize_impl::execute(image, width, height, filter, bounds),
        |bytes| {
            GrayImage::from_raw(width, height, bytes).ok_or_else(|| {
                PilError::ValueError("resize_l: failed to create output buffer".into())
            })
        },
    )
}
