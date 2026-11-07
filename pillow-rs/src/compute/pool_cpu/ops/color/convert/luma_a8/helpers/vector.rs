//! Prepared LA target strategies backed by the archived vector kernels.

use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, Mode, RgbImage, RgbaImage};

pub(crate) fn execute(source: &GrayAlphaImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => {
            let (bytes, blocks) = super::layout::convert(source, 1)?;
            record_vector_output(blocks, bytes.capacity());
            let output =
                GrayImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
                    PilError::InternalError("LA Convert L output shape mismatch".into())
                })?;
            Ok(DynamicImage::ImageLuma8(output))
        }
        Mode::ImageLaPremultiplied8 => {
            let (output, blocks) = super::premultiply::convert(source)?;
            record_archived_alpha_path(blocks, output.as_raw().capacity());
            Ok(DynamicImage::ImageLaPremultiplied8(output))
        }
        Mode::ImageRgb8 => {
            let (bytes, blocks) = super::layout::convert(source, 3)?;
            record_vector_output(blocks, bytes.capacity());
            let output =
                RgbImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
                    PilError::InternalError("LA Convert RGB output shape mismatch".into())
                })?;
            Ok(DynamicImage::ImageRgb8(output))
        }
        Mode::ImageRgba8 => super::expand_color::to_rgba(source).map(DynamicImage::ImageRgba8),
        Mode::ImageRgbx8 => super::expand_color::to_rgbx(source).map(DynamicImage::ImageRgbx8),
        Mode::ImageCmyk8 => {
            let (bytes, blocks) = super::cmyk::convert(source)?;
            record_vector_output(blocks, bytes.capacity());
            let output =
                RgbaImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
                    PilError::InternalError("LA Convert CMYK output shape mismatch".into())
                })?;
            Ok(DynamicImage::ImageCmyk8(output))
        }
        Mode::ImageYCbCr8 => {
            let (bytes, blocks) = super::ycbcr::convert(source)?;
            record_vector_output(blocks, bytes.capacity());
            let output =
                RgbImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
                    PilError::InternalError("LA Convert YCbCr output shape mismatch".into())
                })?;
            Ok(DynamicImage::ImageYCbCr8(output))
        }
        Mode::ImageHsv8 => {
            let (bytes, blocks) = super::hsv::convert(source)?;
            record_vector_output(blocks, bytes.capacity());
            let output =
                RgbImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
                    PilError::InternalError("LA Convert HSV output shape mismatch".into())
                })?;
            Ok(DynamicImage::ImageHsv8(output))
        }
        Mode::ImageI32 => {
            let (bytes, blocks) = super::typed::to_i32(source)?;
            record_vector_output(blocks, bytes.capacity());
            let output =
                RgbaImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
                    PilError::InternalError("LA Convert I output shape mismatch".into())
                })?;
            Ok(DynamicImage::ImageI32(output))
        }
        Mode::ImageF32 => {
            let (bytes, blocks) = super::typed::to_f32(source)?;
            record_vector_output(blocks, bytes.capacity());
            let output =
                RgbaImage::from_raw(source.width(), source.height(), bytes).ok_or_else(|| {
                    PilError::InternalError("LA Convert F output shape mismatch".into())
                })?;
            Ok(DynamicImage::ImageF32(output))
        }
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared LA target".into(),
        )),
    }
}

fn record_vector_output(vector_blocks: u64, allocation_bytes: usize) {
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path(if vector_blocks == 0 {
        "scalar-control"
    } else {
        "vector"
    });
    if allocation_bytes != 0 {
        crate::compute::record_pipeline_allocation(allocation_bytes);
    }
}

fn record_archived_alpha_path(vector_blocks: u64, allocation_bytes: usize) {
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    if allocation_bytes != 0 {
        crate::compute::record_pipeline_allocation(allocation_bytes);
    }
}
