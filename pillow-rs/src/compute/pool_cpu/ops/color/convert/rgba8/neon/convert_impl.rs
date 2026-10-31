//! NEON RGBA-to-RGB conversion for the exact RGBA8 source mode.

use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage, RgbaImage};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => luma8(source),
        Mode::ImageRgb8 => {
            // SAFETY: this wrapper is called only for the selected NEON strategy.
            unsafe { execute_rgb(source) }.map(DynamicImage::ImageRgb8)
        }
        Mode::ImageRgbx8 => super::super::helpers::opaque_padding::execute(source)
            .map(DynamicImage::ImageRgbx8),
        Mode::ImageLumaA8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32 => super::super::helpers::vector::execute(source, target),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGBA NEON target".into(),
        )),
    }
}

fn luma8(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let kernel = crate::compute::pool_cpu::ops::grayscale::rgba8::default::grayscale_impl::prepare(
        source,
    )?;
    let output = crate::compute::pool_cpu::ops::grayscale::rgba8::neon::grayscale_impl::execute(
        kernel,
    );
    let output = crate::compute::pool_cpu::ops::grayscale::helpers::luma::vector_output(
        source.width(),
        source.height(),
        output,
        "vector",
    )?;
    Ok(DynamicImage::ImageLuma8(output))
}

/// Executes the NEON RGBA-to-RGB kernel for an already selected strategy.
///
/// # Safety
///
/// The caller must ensure that the host supports NEON.
pub(crate) unsafe fn execute_rgb(source: &RgbaImage) -> Result<RgbImage, PilError> {
    let pixels = (source.width() as usize)
        .checked_mul(source.height() as usize)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert dimensions overflowed".into()))?;
    let source_bytes = pixels
        .checked_mul(4)
        .ok_or_else(|| PilError::DimensionError("RGBA Convert source size overflowed".into()))?;
    let output_bytes = pixels
        .checked_mul(3)
        .filter(|&bytes| bytes <= isize::MAX as usize)
        .ok_or_else(|| PilError::DimensionError("RGB Convert output size overflowed".into()))?;
    if source.as_raw().len() != source_bytes {
        return Err(PilError::InternalError(
            "RGBA Convert source storage did not match its dimensions".into(),
        ));
    }

    let vector_pixels = pixels / 16 * 16;
    let mut output = Vec::new();
    output
        .try_reserve_exact(output_bytes)
        .map_err(|error| PilError::MemoryError(error.to_string()))?;
    // SAFETY: the source length and destination capacity are exact, and the
    // selected Neon strategy requires the execution context's Neon capability.
    unsafe { rgba_to_rgb_neon(source.as_raw(), vector_pixels, &mut output) };
    for pixel in vector_pixels..pixels {
        let offset = pixel * 4;
        output.extend_from_slice(&source.as_raw()[offset..offset + 3]);
    }

    if pixels == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks((vector_pixels / 16) as u64);
        crate::compute::record_pipeline_operation_scalar_tail((pixels - vector_pixels) as u64);
        crate::compute::record_pipeline_operation_path("neon-rgba-to-rgb");
    }
    RgbImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("RGB Convert output shape mismatch".into()))
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "neon")]
#[allow(unsafe_code)]
unsafe fn rgba_to_rgb_neon(source: &[u8], vector_pixels: usize, output: &mut Vec<u8>) {
    use core::arch::aarch64 as neon;

    // SAFETY: the caller validated four source bytes per pixel and reserved
    // the full destination. Each structure load reads 64 bytes; each store
    // writes 48 bytes for the same sixteen pixels.
    unsafe {
        let output_pointer = output.as_mut_ptr();
        for pixel in (0..vector_pixels).step_by(16) {
            let rgba = neon::vld4q_u8(source.as_ptr().add(pixel * 4));
            let rgb = neon::uint8x16x3_t(rgba.0, rgba.1, rgba.2);
            neon::vst3q_u8(output_pointer.add(pixel * 3), rgb);
        }
        output.set_len(vector_pixels * 3);
    }
}
