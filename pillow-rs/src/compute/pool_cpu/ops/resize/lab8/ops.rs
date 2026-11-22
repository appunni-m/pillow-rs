//! Exact lab8 mode selection for Resize.

use crate::compute::registry::support::{ImplementationSupport, ModeSupport, StrategySelection};
use crate::compute::{Architecture, ExecutionContext};
use crate::error::PilError;
use crate::pipeline::ResampleFilter;
use crate::raster::{Mode, RgbImage};

static RESIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::support::native_byte_architectures(Mode::ImageLab8, 3, false);
static RESIZE_BOXED_ARCHITECTURES: [ImplementationSupport; 3] =
    super::super::helpers::support::boxed_byte_architectures(Mode::ImageLab8, 3, false);

pub(crate) const SUPPORT: ModeSupport =
    super::super::helpers::support::native_mode_support(Mode::ImageLab8, &RESIZE_ARCHITECTURES);
pub(crate) const BOXED_SUPPORT: ModeSupport = super::super::helpers::support::boxed_mode_support(
    Mode::ImageLab8,
    &RESIZE_BOXED_ARCHITECTURES,
);

// VERIFY: source=reviewed@2026-11-20; parity=pass@migration-parity-5e339e0b1e114b5ea54221d8961f1b46; benchmark=pending
pub(crate) fn resize(
    context: &ExecutionContext,
    selection: &StrategySelection,
    image: &RgbImage,
    width: u32,
    height: u32,
    filter: ResampleFilter,
    bounds: Option<(f64, f64, f64, f64)>,
) -> Result<RgbImage, PilError> {
    let architecture = selection.implementation.architecture;
    let kernel_image = kernel_image(image)?;
    super::super::helpers::strategy::resize_byte_native::<_, 3>(
        context,
        selection,
        kernel_image.as_raw(),
        kernel_image.dimensions(),
        width,
        height,
        filter,
        bounds,
        || {
            let output =
                super::default::resize_impl::execute(&kernel_image, width, height, filter, bounds)?;
            output_image(width, height, output.into_raw())
        },
        |bounds| match architecture {
            Architecture::Sse2 => match bounds {
                Some(bounds) => super::sse2::resize_impl::execute_boxed(
                    &kernel_image,
                    width,
                    height,
                    bounds,
                    filter,
                ),
                None => super::sse2::resize_impl::execute(&kernel_image, width, height, filter),
            },
            Architecture::Neon => match bounds {
                Some(bounds) => super::neon::resize_impl::execute_boxed(
                    &kernel_image,
                    width,
                    height,
                    bounds,
                    filter,
                ),
                None => super::neon::resize_impl::execute(&kernel_image, width, height, filter),
            },
            Architecture::WasmSimd128 => match bounds {
                Some(bounds) => super::wasm_simd128::resize_impl::execute_boxed(
                    &kernel_image,
                    width,
                    height,
                    bounds,
                    filter,
                ),
                None => {
                    super::wasm_simd128::resize_impl::execute(&kernel_image, width, height, filter)
                }
            },
            _ => Err(PilError::InternalError(format!(
                "lab8 Resize received unsupported architecture {architecture:?}"
            ))),
        },
        |bytes| output_image(width, height, bytes),
    )
}

// VERIFY: source=reviewed@2026-11-20; parity=pass@migration-parity-5e339e0b1e114b5ea54221d8961f1b46; benchmark=pending
fn kernel_image(image: &RgbImage) -> Result<RgbImage, PilError> {
    let (width, height) = image.dimensions();
    let mut samples = image.as_raw().clone();
    toggle_lab_chroma_bias(&mut samples);
    RgbImage::from_raw(width, height, samples)
        .ok_or_else(|| PilError::InternalError("invalid LAB Resize input buffer".into()))
}

// VERIFY: source=reviewed@2026-11-20; parity=pass@migration-parity-5e339e0b1e114b5ea54221d8961f1b46; benchmark=pending
fn output_image(width: u32, height: u32, mut samples: Vec<u8>) -> Result<RgbImage, PilError> {
    toggle_lab_chroma_bias(&mut samples);
    RgbImage::from_raw(width, height, samples)
        .ok_or_else(|| PilError::ValueError("resize_lab: failed to create output buffer".into()))
}

// VERIFY: source=reviewed@2026-11-20; parity=pass@migration-parity-5e339e0b1e114b5ea54221d8961f1b46; benchmark=pending
fn toggle_lab_chroma_bias(samples: &mut [u8]) {
    // This Resize-only byte mapping preserves the frozen Pillow output. It is
    // not the public LAB byte representation used by pixel and band APIs.
    for pixel in samples.chunks_exact_mut(3) {
        pixel[1] ^= 0x80;
        pixel[2] ^= 0x80;
    }
}
