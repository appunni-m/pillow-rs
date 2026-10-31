//! Shared portable wide-vector target kernels for the Luma8 architecture routes.

use crate::error::PilError;
use crate::raster::{
    DynamicImage, GrayAlphaImage, GrayImage, Mode, RgbImage, RgbaImage,
};
use wide::u8x16;

const IDENTITY_LANES: u8x16 =
    u8x16::new([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);

pub(crate) fn execute(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    match target {
        Mode::ImageLuma8 => identity(source),
        Mode::ImageRgb8 => super::expand_color::to_rgb(source).map(DynamicImage::ImageRgb8),
        Mode::ImageRgba8 => super::expand_color::to_rgba(source).map(DynamicImage::ImageRgba8),
        Mode::ImageLumaA8 => luma_alpha(source),
        Mode::ImageI16 | Mode::ImageI16L | Mode::ImageI16B | Mode::ImageI16N => {
            luma16(source, target)
        }
        Mode::ImageCmyk8 => cmyk(source),
        Mode::ImageYCbCr8 => ycbcr(source),
        Mode::ImageHsv8 => hsv(source),
        Mode::ImageI32 => integer(source),
        Mode::ImageF32 => float(source),
        _ => Err(PilError::InternalError(
            "Convert selected an undeclared Luma8 architecture".into(),
        )),
    }
}

fn identity(source: &GrayImage) -> Result<DynamicImage, PilError> {
    let (pixels, output_bytes) = super::layout::checked_layout(source, 1)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut blocks = 0u64;
    for start in (0..pixels).step_by(16) {
        let active = (pixels - start).min(16);
        let lanes = std::array::from_fn(|lane| {
            source.as_raw().get(start + lane).copied().unwrap_or(0)
        });
        let block = u8x16::new(lanes).swizzle_relaxed(IDENTITY_LANES).to_array();
        output.extend_from_slice(&block[..active]);
        blocks = blocks.saturating_add(1);
    }
    record_vector_output(blocks, output.capacity());
    let image = GrayImage::from_raw(source.width(), source.height(), output)
        .ok_or_else(|| PilError::InternalError("Luma Convert L output shape mismatch".into()))?;
    Ok(DynamicImage::ImageLuma8(image))
}

fn luma_alpha(source: &GrayImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = super::luma_alpha::execute(source)?;
    record_vector_output(blocks, bytes.capacity());
    let output = GrayAlphaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Luma Convert LA output shape mismatch".into()))?;
    Ok(DynamicImage::ImageLumaA8(output))
}

fn luma16(source: &GrayImage, target: Mode) -> Result<DynamicImage, PilError> {
    let (output, blocks) = super::typed::to_i16(source)?;
    crate::compute::record_pipeline_operation_vector_blocks(blocks);
    crate::compute::record_pipeline_operation_scalar_tail(0);
    crate::compute::record_pipeline_operation_path("vector");
    record_sample_allocation(output.as_raw().capacity());
    match target {
        Mode::ImageI16 => Ok(DynamicImage::ImageI16(output)),
        Mode::ImageI16L => Ok(DynamicImage::ImageI16L(output)),
        Mode::ImageI16B => Ok(DynamicImage::ImageI16B(output)),
        Mode::ImageI16N => Ok(DynamicImage::ImageI16N(output)),
        _ => Err(PilError::InternalError("invalid Luma8 I;16 target".into())),
    }
}

fn cmyk(source: &GrayImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = super::cmyk::execute(source)?;
    record_vector_output(blocks, bytes.capacity());
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Luma Convert CMYK output shape mismatch".into()))?;
    Ok(DynamicImage::ImageCmyk8(output))
}

fn ycbcr(source: &GrayImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = super::ycbcr::execute(source)?;
    record_vector_output(blocks, bytes.capacity());
    let output = RgbImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Luma Convert YCbCr output shape mismatch".into()))?;
    Ok(DynamicImage::ImageYCbCr8(output))
}

fn hsv(source: &GrayImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = super::hsv::execute(source)?;
    record_vector_output(blocks, bytes.capacity());
    let output = RgbImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Luma Convert HSV output shape mismatch".into()))?;
    Ok(DynamicImage::ImageHsv8(output))
}

fn integer(source: &GrayImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = super::typed::to_i32(source)?;
    record_vector_output(blocks, bytes.capacity());
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Luma Convert I output shape mismatch".into()))?;
    Ok(DynamicImage::ImageI32(output))
}

fn float(source: &GrayImage) -> Result<DynamicImage, PilError> {
    let (bytes, blocks) = super::typed::to_f32(source)?;
    record_vector_output(blocks, bytes.capacity());
    let output = RgbaImage::from_raw(source.width(), source.height(), bytes)
        .ok_or_else(|| PilError::InternalError("Luma Convert F output shape mismatch".into()))?;
    Ok(DynamicImage::ImageF32(output))
}

fn record_sample_allocation(sample_capacity: usize) {
    let allocation = sample_capacity.saturating_mul(std::mem::size_of::<u16>());
    if allocation != 0 {
        crate::compute::record_pipeline_allocation(allocation);
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
