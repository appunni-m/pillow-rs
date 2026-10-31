//! Direct RGBa unpremultiplication kernels for straight-alpha Convert targets.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode, RgbImage, RgbaImage};
use wide::u8x16;

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLaPremultiplied8 => convert_luma_alpha(source),
        Mode::ImageRgb8 => convert_rgb(source),
        Mode::ImageRgba8 => convert_rgba(source),
        Mode::ImageRgbx8 => convert_rgbx(source),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGBa alpha target".into(),
        )),
    }
}

fn convert_luma_alpha(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = transform_blocks(source, 2, append_luma_alpha)?;
    let allocation = bytes.capacity();
    let output = GrayAlphaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGBa Convert La output shape mismatch".into()))?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageLaPremultiplied8(output))
}

fn convert_rgb(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = transform_blocks(source, 3, append_rgb)?;
    let allocation = bytes.capacity();
    let output = RgbImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGBa Convert RGB output shape mismatch".into()))?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageRgb8(output))
}

fn convert_rgba(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = transform_blocks(source, 4, append_rgba)?;
    let allocation = bytes.capacity();
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGBa Convert RGBA output shape mismatch".into()))?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageRgba8(output))
}

fn convert_rgbx(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = transform_blocks(source, 4, append_rgbx)?;
    let allocation = bytes.capacity();
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGBa Convert RGBX output shape mismatch".into()))?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageRgbx8(output))
}

fn transform_blocks<F>(
    source: &RgbaImage,
    output_channels: u8,
    mut append: F,
) -> Result<(Vec<u8>, u64), PilError>
where
    F: FnMut(&[u8; 16], usize, &mut Vec<u8>),
{
    let source_dims = CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let output_dims = CheckedDims::new_allow_empty(source.width(), source.height(), output_channels)?;
    if source.as_raw().len() != source_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBa Convert source storage did not match its dimensions".into(),
        ));
    }
    let mut output = Vec::with_capacity(output_dims.total_bytes());
    let mut vector_blocks = 0u64;
    for block in source.as_raw().chunks(16) {
        let active_pixels = block.len() / 4;
        let mut padded = [0u8; 16];
        padded[..block.len()].copy_from_slice(block);
        let converted =
            crate::compute::pool_cpu::ops::color::convert::helpers::alpha::unpremultiply_block::<4>(
                u8x16::new(padded),
            )
            .to_array();
        append(&converted, active_pixels, &mut output);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_dims.total_bytes() {
        return Err(PilError::InternalError(
            "RGBa Convert output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}

fn append_luma_alpha(
    pixels: &[u8; 16],
    active_pixels: usize,
    output: &mut Vec<u8>,
) {
    for pixel in pixels.chunks_exact(4).take(active_pixels) {
        let luma = crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2]);
        output.extend_from_slice(&[luma, u8::MAX]);
    }
}

fn append_rgb(pixels: &[u8; 16], active_pixels: usize, output: &mut Vec<u8>) {
    for pixel in pixels.chunks_exact(4).take(active_pixels) {
        output.extend_from_slice(&pixel[..3]);
    }
}

fn append_rgba(pixels: &[u8; 16], active_pixels: usize, output: &mut Vec<u8>) {
    output.extend_from_slice(&pixels[..active_pixels * 4]);
}

fn append_rgbx(pixels: &[u8; 16], active_pixels: usize, output: &mut Vec<u8>) {
    for pixel in pixels.chunks_exact(4).take(active_pixels) {
        output.extend_from_slice(&[pixel[0], pixel[1], pixel[2], u8::MAX]);
    }
}

fn record_vector_output(vector_blocks: u64, allocation_bytes: usize) {
    if vector_blocks == 0 {
        crate::compute::record_pipeline_operation_path("scalar-control");
    } else {
        crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
        crate::compute::record_pipeline_operation_scalar_tail(0);
        crate::compute::record_pipeline_operation_path("vector");
    }
    if allocation_bytes != 0 {
        crate::compute::record_pipeline_allocation(allocation_bytes);
    }
}
