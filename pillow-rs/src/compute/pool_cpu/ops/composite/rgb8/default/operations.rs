//! Serial native-sample implementations for ImageRgb8.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

const MODE: Mode = Mode::ImageRgb8;

pub(crate) fn merge(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    if source.mode() == Mode::ImageLuma8
        && remaining.len() == 2
        && remaining
            .iter()
            .all(|band| band.mode() == Mode::ImageLuma8)
    {
        return interleave_luma_bands(source, remaining);
    }
    crate::compute::pool_cpu::ops::composite::helpers::merge::interleave(
        source,
        remaining,
        MODE,
        crate::compute::pool_cpu::ops::composite::ops::MergeLayout::Three { lab_encoding: false },
    )
}

fn interleave_luma_bands(
    source: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
) -> Result<DynamicImage, PilError> {
    let (width, height) = (source.width(), source.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, 3)?;
    let pixels = dimensions.total_bytes() / 3;
    let bands = crate::compute::pool_cpu::ops::composite::helpers::merge::collect_bands(
        source,
        remaining,
        pixels,
    )?;
    if bands.len() != 3 {
        return Err(PilError::InternalError(
            "RGB Merge luma path requires three bands".into(),
        ));
    }
    let output = interleave_rgb_luma_bands(bands[0], bands[1], bands[2])?;
    let mut output = crate::compute::pool_cpu::ops::composite::helpers::native_bytes::construct(
        width, height, MODE, output,
    )?;
    crate::compute::pool_cpu::ops::composite::helpers::merge::preserve_merged_padding(
        &mut output,
        MODE,
        crate::compute::pool_cpu::ops::composite::ops::MergeLayout::Three {
            lab_encoding: false,
        },
        pixels,
    )?;
    Ok(output)
}

fn interleave_rgb_luma_bands(
    red: &[u8],
    green: &[u8],
    blue: &[u8],
) -> Result<Vec<u8>, PilError> {
    if red.len() != green.len() || red.len() != blue.len() {
        return Err(PilError::InternalError(
            "RGB merge band lengths do not match".into(),
        ));
    }
    let output_len = red
        .len()
        .checked_mul(3)
        .ok_or_else(|| PilError::ValueError("merge: output length overflow".into()))?;
    const PIXELS_PER_CHUNK: usize = 64;
    let mut output = Vec::with_capacity(output_len);
    let mut red_chunks = red.chunks_exact(PIXELS_PER_CHUNK);
    let mut green_chunks = green.chunks_exact(PIXELS_PER_CHUNK);
    let mut blue_chunks = blue.chunks_exact(PIXELS_PER_CHUNK);
    for ((red_chunk, green_chunk), blue_chunk) in red_chunks
        .by_ref()
        .zip(green_chunks.by_ref())
        .zip(blue_chunks.by_ref())
    {
        let mut pixels = [[0; 3]; PIXELS_PER_CHUNK];
        for ((pixel, &red), (&green, &blue)) in pixels
            .iter_mut()
            .zip(red_chunk)
            .zip(green_chunk.iter().zip(blue_chunk))
        {
            *pixel = [red, green, blue];
        }
        output.extend_from_slice(pixels.as_flattened());
    }
    for ((&red, &green), &blue) in red_chunks
        .remainder()
        .iter()
        .zip(green_chunks.remainder())
        .zip(blue_chunks.remainder())
    {
        output.extend([red, green, blue]);
    }
    Ok(output)
}
