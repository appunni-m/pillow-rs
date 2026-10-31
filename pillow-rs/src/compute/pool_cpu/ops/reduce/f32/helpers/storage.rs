//! Exact F-mode sample viewing and output construction for architecture kernels.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::ops::pil_resize::f32_samples_from_le_bytes;
use crate::raster::{DynamicImage, RgbaImage};
use std::borrow::Cow;

pub(crate) fn source_samples(image: &RgbaImage) -> Result<Cow<'_, [f32]>, PilError> {
    let source_bytes = CheckedDims::new(image.width(), image.height(), 4)?.total_bytes();
    if image.as_raw().len() != source_bytes {
        return Err(PilError::InternalError(
            "ImageF32 Reduce received an invalid scalar carrier".into(),
        ));
    }
    let samples = f32_samples_from_le_bytes(image.as_raw(), source_bytes / 4);
    if samples.len() != source_bytes / 4 {
        return Err(PilError::InternalError(
            "ImageF32 Reduce could not read all scalar samples".into(),
        ));
    }
    Ok(samples)
}

pub(crate) fn output_capacity(width: u32, height: u32) -> Result<usize, PilError> {
    Ok(CheckedDims::new(width, height, 4)?.total_bytes())
}

pub(crate) fn append_samples(output: &mut Vec<u8>, samples: &[f32]) {
    #[cfg(target_endian = "little")]
    output.extend_from_slice(bytemuck::cast_slice(samples));
    #[cfg(target_endian = "big")]
    for sample in samples {
        output.extend_from_slice(&sample.to_le_bytes());
    }
}

pub(crate) fn finish_output(
    width: u32,
    height: u32,
    bytes: Vec<u8>,
) -> Result<DynamicImage, PilError> {
    let expected_bytes = output_capacity(width, height)?;
    if bytes.len() != expected_bytes {
        return Err(PilError::InternalError(
            "ImageF32 Reduce produced an invalid output length".into(),
        ));
    }
    let image = RgbaImage::from_raw(width, height, bytes).ok_or_else(|| {
        PilError::InternalError("failed to construct ImageF32 Reduce output".into())
    })?;
    Ok(DynamicImage::ImageF32(image))
}
