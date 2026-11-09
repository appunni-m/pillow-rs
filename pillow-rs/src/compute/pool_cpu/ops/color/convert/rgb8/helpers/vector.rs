//! Shared RGB8 output construction for architecture-selected conversions.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, Mode, RgbImage, RgbaImage};

pub(crate) fn luma_alpha(source: &RgbImage, target: Mode) -> Result<DynamicImage, PilError> {
    let (bytes, vector_blocks) = super::luma_alpha::execute(source)?;
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path(if vector_blocks == 0 {
        "scalar-control"
    } else {
        "vector"
    });
    if bytes.capacity() != 0 {
        crate::compute::record_pipeline_allocation(bytes.capacity());
    }
    let output =
        GrayAlphaImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
            PilError::InternalError("RGB Convert luma-alpha output shape mismatch".into())
        })?;
    match target {
        Mode::ImageLumaA8 => Ok(DynamicImage::ImageLumaA8(output)),
        Mode::ImageLaPremultiplied8 => Ok(DynamicImage::ImageLaPremultiplied8(output)),
        _ => Err(PilError::InternalError("invalid RGB8 alpha target".into())),
    }
}

pub(crate) fn rgba_for_target(output: RgbaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageRgba8 => Ok(DynamicImage::ImageRgba8(output)),
        Mode::ImageRgbPremultiplied8 => Ok(DynamicImage::ImageRgbPremultiplied8(output)),
        Mode::ImageRgbx8 => Ok(DynamicImage::ImageRgbx8(output)),
        _ => Err(PilError::InternalError(
            "invalid RGB8 four-channel target".into(),
        )),
    }
}

pub(crate) fn cmyk(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let (bytes, vector_blocks) =
        crate::compute::pool_cpu::ops::color::convert::helpers::native_rgb::to_cmyk::<3>(
            source.as_raw(),
            source.width(),
            source.height(),
            Mode::ImageRgb8,
        )?;
    record_vector_output(vector_blocks, bytes.capacity());
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGB Convert CMYK output shape mismatch".into()))?;
    Ok(DynamicImage::ImageCmyk8(output))
}

pub(crate) fn ycbcr(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let (bytes, vector_blocks) =
        crate::compute::pool_cpu::ops::color::convert::helpers::native_rgb::to_ycbcr::<3>(
            source.as_raw(),
            source.width(),
            source.height(),
            Mode::ImageRgb8,
        )?;
    record_vector_output(vector_blocks, bytes.capacity());
    let output = RgbImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGB Convert YCbCr output shape mismatch".into()))?;
    Ok(DynamicImage::ImageYCbCr8(output))
}

pub(crate) fn hsv(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let (bytes, vector_blocks) =
        crate::compute::pool_cpu::ops::color::convert::helpers::native_rgb::to_hsv::<3>(
            source.as_raw(),
            source.width(),
            source.height(),
            Mode::ImageRgb8,
        )?;
    record_vector_output(vector_blocks, bytes.capacity());
    let output = RgbImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGB Convert HSV output shape mismatch".into()))?;
    Ok(DynamicImage::ImageHsv8(output))
}

pub(crate) fn i32(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let (bytes, vector_blocks) =
        crate::compute::pool_cpu::ops::color::convert::helpers::native_rgb::to_i32::<3>(
            source.as_raw(),
            source.width(),
            source.height(),
            Mode::ImageRgb8,
        )?;
    record_vector_output(vector_blocks, bytes.capacity());
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGB Convert I output shape mismatch".into()))?;
    Ok(DynamicImage::ImageI32(output))
}

pub(crate) fn f32(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let (bytes, vector_blocks) =
        crate::compute::pool_cpu::ops::color::convert::helpers::native_rgb::to_f32::<3>(
            source.as_raw(),
            source.width(),
            source.height(),
            Mode::ImageRgb8,
        )?;
    record_vector_output(vector_blocks, bytes.capacity());
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("RGB Convert F output shape mismatch".into()))?;
    Ok(DynamicImage::ImageF32(output))
}

pub(crate) fn identity(source: &RgbImage) -> Result<DynamicImage, PilError> {
    let (bytes, vector_blocks) = super::layout::execute(source)?;
    record_vector_output(vector_blocks, bytes.capacity());
    let output = RgbImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
        PilError::InternalError("RGB Convert identity output shape mismatch".into())
    })?;
    Ok(DynamicImage::ImageRgb8(output))
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
