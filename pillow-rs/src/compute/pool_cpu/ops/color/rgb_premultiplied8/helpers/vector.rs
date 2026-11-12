//! Shared direct RGBa-to-target routes selected by SSE2 or NEON context.

use crate::compute::pool_cpu::ops::color::helpers::convert::native_rgb;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode, RgbImage, RgbaImage};

pub(crate) fn execute(source: &RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLumaA8 => convert_luma_alpha(source),
        Mode::ImageLaPremultiplied8 | Mode::ImageRgb8 | Mode::ImageRgba8 | Mode::ImageRgbx8 => {
            super::alpha::execute(source, target)
        }
        Mode::ImageCmyk8 => convert_cmyk(source),
        Mode::ImageYCbCr8 => convert_ycbcr(source),
        Mode::ImageHsv8 => convert_hsv(source),
        Mode::ImageI32 => convert_i32(source),
        Mode::ImageF32 => convert_f32(source),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared RGBa vector target".into(),
        )),
    }
}

fn convert_luma_alpha(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (output, blocks, allocation) = super::luma_alpha::execute(source)?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageLumaA8(output))
}

fn convert_cmyk(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = native_rgb::to_cmyk::<4>(
        source.as_raw(),
        source.width(),
        source.height(),
        Mode::ImageRgbPremultiplied8,
    )?;
    let allocation = bytes.capacity();
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGBa Convert CMYK output shape mismatch".into()))?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageCmyk8(output))
}

fn convert_ycbcr(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = native_rgb::to_ycbcr::<4>(
        source.as_raw(),
        source.width(),
        source.height(),
        Mode::ImageRgbPremultiplied8,
    )?;
    let allocation = bytes.capacity();
    let output = RgbImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
        PilError::InternalError("RGBa Convert YCbCr output shape mismatch".into())
    })?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageYCbCr8(output))
}

fn convert_hsv(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = native_rgb::to_hsv::<4>(
        source.as_raw(),
        source.width(),
        source.height(),
        Mode::ImageRgbPremultiplied8,
    )?;
    let allocation = bytes.capacity();
    let output = RgbImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGBa Convert HSV output shape mismatch".into()))?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageHsv8(output))
}

fn convert_i32(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = native_rgb::to_i32::<4>(
        source.as_raw(),
        source.width(),
        source.height(),
        Mode::ImageRgbPremultiplied8,
    )?;
    let allocation = bytes.capacity();
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGBa Convert I output shape mismatch".into()))?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageI32(output))
}

fn convert_f32(source: &RgbaImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = native_rgb::to_f32::<4>(
        source.as_raw(),
        source.width(),
        source.height(),
        Mode::ImageRgbPremultiplied8,
    )?;
    let allocation = bytes.capacity();
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGBa Convert F output shape mismatch".into()))?;
    record_vector_output(blocks, allocation);
    Ok(DynamicImage::ImageF32(output))
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
