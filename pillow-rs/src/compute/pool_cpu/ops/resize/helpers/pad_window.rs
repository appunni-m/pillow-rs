//! Mode/architecture dispatch for prepared Resize output windows.

use crate::compute::Architecture;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, Mode};

pub(crate) fn supports(
    image: &DynamicImage,
    selection: &StrategySelection,
    filtered: bool,
) -> bool {
    let architecture = selection.implementation.architecture;
    filtered
        && matches!(image.mode(), Mode::ImageLuma8 | Mode::ImageRgbx8)
        && matches!(
            architecture,
            Architecture::Default
                | Architecture::Sse2
                | Architecture::Neon
                | Architecture::WasmSimd128
        )
}

pub(crate) fn execute(
    image: &DynamicImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    selection: &StrategySelection,
    output: &mut [u8],
) -> Result<bool, PilError> {
    let architecture = selection.implementation.architecture;
    match (architecture, image) {
        (Architecture::Default, DynamicImage::ImageLuma8(image)) => Ok(
            super::super::luma8::default::resize_impl::execute_into_window(
                image, width, height, filter, output,
            ),
        ),
        (Architecture::Sse2, DynamicImage::ImageLuma8(image)) => {
            Ok(super::super::luma8::sse2::resize_impl::execute_into_window(
                image, width, height, filter, output,
            ))
        }
        (Architecture::Neon, DynamicImage::ImageLuma8(image)) => {
            Ok(super::super::luma8::neon::resize_impl::execute_into_window(
                image, width, height, filter, output,
            ))
        }
        (Architecture::WasmSimd128, DynamicImage::ImageLuma8(image)) => Ok(
            super::super::luma8::wasm_simd128::resize_impl::execute_into_window(
                image, width, height, filter, output,
            ),
        ),
        (Architecture::Default, DynamicImage::ImageRgbx8(image)) => {
            super::super::rgbx8::default::resize_impl::execute_into_window(
                image, width, height, filter, output,
            )
        }
        (Architecture::Sse2, DynamicImage::ImageRgbx8(image)) => {
            super::super::rgbx8::sse2::resize_impl::execute_into_window(
                image, width, height, filter, output,
            )
        }
        (Architecture::Neon, DynamicImage::ImageRgbx8(image)) => {
            super::super::rgbx8::neon::resize_impl::execute_into_window(
                image, width, height, filter, output,
            )
        }
        (Architecture::WasmSimd128, DynamicImage::ImageRgbx8(image)) => {
            super::super::rgbx8::wasm_simd128::resize_impl::execute_into_window(
                image, width, height, filter, output,
            )
        }
        _ => Ok(false),
    }
}
