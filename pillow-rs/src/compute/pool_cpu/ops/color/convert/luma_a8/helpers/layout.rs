//! Native LA byte-layout conversion from the archived portable vector kernel.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::GrayAlphaImage;
use wide::u8x16;

pub(crate) fn convert(
    source: &GrayAlphaImage,
    target_channels: usize,
) -> Result<(Vec<u8>, u64), PilError> {
    if !(1..=4).contains(&target_channels) {
        return Err(PilError::InternalError(
            "LA Convert target has unsupported byte width".into(),
        ));
    }
    let dims = CheckedDims::new_allow_empty(source.width(), source.height(), 2)?;
    if source.as_raw().len() != dims.total_bytes() {
        return Err(PilError::InternalError(
            "LA Convert source storage did not match its dimensions".into(),
        ));
    }
    let output_bytes = dims
        .total_pixels()
        .checked_mul(target_channels)
        .ok_or_else(|| PilError::DimensionError("LA Convert output size overflowed".into()))?;
    let pixels_per_vector = 16 / 2usize.max(target_channels);
    let mut output = Vec::with_capacity(output_bytes);
    let mut vector_blocks = 0u64;
    for start_pixel in (0..dims.total_pixels()).step_by(pixels_per_vector) {
        let active = (dims.total_pixels() - start_pixel).min(pixels_per_vector);
        let source_start = start_pixel * 2;
        let input_bytes = active * 2;
        let mut padded = [0u8; 16];
        padded[..input_bytes]
            .copy_from_slice(&source.as_raw()[source_start..source_start + input_bytes]);
        let mut indices = [0u8; 16];
        for pixel in 0..pixels_per_vector {
            for channel in 0..target_channels {
                let carries_alpha =
                    (target_channels == 2 && channel == 1)
                        || (target_channels == 4 && channel == 3);
                let source_channel = if carries_alpha { 1 } else { 0 };
                indices[pixel * target_channels + channel] =
                    u8::try_from(pixel * 2 + source_channel).map_err(|_| {
                        PilError::InternalError("LA Convert shuffle index overflowed".into())
                    })?;
            }
        }
        let lanes = u8x16::new(padded)
            .shuffle(u8x16::new(indices))
            .to_array();
        output.extend_from_slice(&lanes[..active * target_channels]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    if output.len() != output_bytes {
        return Err(PilError::InternalError(
            "LA Convert layout output shape mismatch".into(),
        ));
    }
    Ok((output, vector_blocks))
}
