//! Native Luma8 to I/F scalar conversion from the archived typed kernels.

use crate::error::PilError;
use crate::raster::{GrayImage, ImageBuffer, Luma};
use wide::{f32x8, u16x16, u32x8, u8x16};

pub(crate) fn to_i32(source: &GrayImage) -> Result<(Vec<u8>, u64), PilError> {
    let (pixels, output_bytes) = super::layout::checked_layout(source, 4)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let luma = std::array::from_fn(|lane| {
            if lane < active {
                u32::from(source.as_raw()[start + lane])
            } else {
                0
            }
        });
        let values = u32x8::new(luma).to_array().map(|value| value as i32);
        for value in values.into_iter().take(active) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError("Luma Convert I output shape mismatch".into()));
    }
    Ok((output, vector_blocks))
}

pub(crate) fn to_f32(source: &GrayImage) -> Result<(Vec<u8>, u64), PilError> {
    let (pixels, output_bytes) = super::layout::checked_layout(source, 4)?;
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(8) {
        let active = (pixels - start).min(8);
        let luma = std::array::from_fn(|lane| {
            if lane < active {
                f32::from(source.as_raw()[start + lane])
            } else {
                0.0
            }
        });
        for value in f32x8::new(luma).to_array().into_iter().take(active) {
            output.extend_from_slice(&value.to_le_bytes());
        }
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError("Luma Convert F output shape mismatch".into()));
    }
    Ok((output, vector_blocks))
}

pub(crate) fn to_i16(
    source: &GrayImage,
) -> Result<(ImageBuffer<Luma<u16>, Vec<u16>>, u64), PilError> {
    let (pixels, _) = super::layout::checked_layout(source, 2)?;
    let mut samples = Vec::with_capacity(pixels);
    let mut vector_blocks = 0u64;
    for start in (0..pixels).step_by(16) {
        let active = (pixels - start).min(16);
        let lanes = std::array::from_fn(|lane| {
            source.as_raw().get(start + lane).copied().unwrap_or(0)
        });
        samples.extend(u16x16::from(u8x16::new(lanes)).to_array().into_iter().take(active));
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if samples.len() != pixels {
        return Err(PilError::InternalError(
            "Luma Convert I;16 output shape mismatch".into(),
        ));
    }
    let image = ImageBuffer::from_raw(source.width(), source.height(), samples)
        .ok_or_else(|| PilError::InternalError("Luma Convert I;16 output shape mismatch".into()))?;
    Ok((image, vector_blocks))
}
