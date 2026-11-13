//! Shared byte interleave kernels for exact Merge layouts.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::buffer::HiddenPixelLanes;
use crate::raster::{DynamicImage, Mode};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub(crate) enum MergeLayout {
    Two,
    Three { lab_encoding: bool },
    Four,
}

impl MergeLayout {
    // VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
    pub(crate) const fn channels(self) -> u8 {
        match self {
            Self::Two => 2,
            Self::Three { .. } => 3,
            Self::Four => 4,
        }
    }
}

pub(crate) fn interleave(
    first: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    mode: Mode,
    layout: MergeLayout,
) -> Result<DynamicImage, PilError> {
    let (width, height) = (first.width(), first.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, layout.channels())?;
    let pixels = dimensions.total_bytes() / usize::from(layout.channels());
    let bands = collect_bands(first, remaining, pixels)?;
    let mut output = dimensions.alloc_buffer();

    match layout {
        MergeLayout::Two => interleave_two(&bands, &mut output),
        MergeLayout::Three { lab_encoding } => interleave_three(&bands, &mut output, lab_encoding),
        MergeLayout::Four => interleave_four(&bands, &mut output),
    }

    let mut output = crate::compute::pool_cpu::ops::composite::helpers::native_bytes::construct(
        width, height, mode, output,
    )?;
    preserve_merged_padding(&mut output, mode, layout, pixels)?;
    Ok(output)
}

pub(crate) fn collect_bands<'a>(
    first: &'a DynamicImage,
    remaining: &'a [Arc<DynamicImage>],
    pixels: usize,
) -> Result<Vec<&'a [u8]>, PilError> {
    let mut bands = Vec::with_capacity(remaining.len() + 1);
    bands.push(first.as_bytes());
    for band in remaining {
        if band.as_bytes().len() != pixels {
            return Err(PilError::InternalError(
                "Merge band storage does not match its dimensions".into(),
            ));
        }
        bands.push(band.as_bytes());
    }
    if bands[0].len() != pixels {
        return Err(PilError::InternalError(
            "first Merge band storage does not match its dimensions".into(),
        ));
    }
    Ok(bands)
}

fn interleave_two(bands: &[&[u8]], output: &mut [u8]) {
    for (index, pixel) in output.chunks_exact_mut(2).enumerate() {
        pixel[0] = bands[0][index];
        pixel[1] = bands[1][index];
    }
}

fn interleave_three(bands: &[&[u8]], output: &mut [u8], lab_encoding: bool) {
    for (index, pixel) in output.chunks_exact_mut(3).enumerate() {
        pixel[0] = bands[0][index];
        pixel[1] = if lab_encoding {
            bands[1][index].wrapping_add(128)
        } else {
            bands[1][index]
        };
        pixel[2] = if lab_encoding {
            bands[2][index].wrapping_add(128)
        } else {
            bands[2][index]
        };
    }
}

/// Retains zero-filled physical padding written by Pillow's band interleave.
pub(crate) fn preserve_merged_padding(
    output: &mut DynamicImage,
    mode: Mode,
    layout: MergeLayout,
    pixel_count: usize,
) -> Result<(), PilError> {
    let lane_count = match layout {
        MergeLayout::Two => 2,
        MergeLayout::Three { .. } => 1,
        MergeLayout::Four => 0,
    };
    let Some(expected_lanes) = mode.hidden_pixel_lane_count() else {
        return if lane_count == 0 {
            Ok(())
        } else {
            Err(PilError::InternalError(
                "Merge layout declared omitted lanes for a full-storage mode".into(),
            ))
        };
    };
    if lane_count != expected_lanes {
        return Err(PilError::InternalError(
            "Merge layout did not match the mode's omitted lanes".into(),
        ));
    }
    if pixel_count == 0 {
        return Ok(());
    }
    let lanes = HiddenPixelLanes::constant(expected_lanes, pixel_count, [0, 0])
        .ok_or_else(|| PilError::InternalError("Merge lane extent was invalid".into()))?;
    if !output.replace_hidden_pixel_lanes(Some(lanes)) {
        return Err(PilError::InternalError(
            "Merge output lanes did not match its mode".into(),
        ));
    }
    Ok(())
}

fn interleave_four(bands: &[&[u8]], output: &mut [u8]) {
    for (index, pixel) in output.chunks_exact_mut(4).enumerate() {
        pixel[0] = bands[0][index];
        pixel[1] = bands[1][index];
        pixel[2] = bands[2][index];
        pixel[3] = bands[3][index];
    }
}

/// Interleaves canonical native L bands with the archived portable 16-byte
/// shuffle, retaining a scalar tail for every channel count.
pub(crate) fn interleave_wide(
    first: &DynamicImage,
    remaining: &[Arc<DynamicImage>],
    mode: Mode,
    layout: MergeLayout,
) -> Result<DynamicImage, PilError> {
    let channels = usize::from(layout.channels());
    let (width, height) = (first.width(), first.height());
    let dimensions = CheckedDims::new_allow_empty(width, height, layout.channels())?;
    let pixels = dimensions.total_bytes() / channels;
    let bands = collect_bands(first, remaining, pixels)?;
    if bands.len() != channels {
        return Err(PilError::InternalError(
            "portable Merge shuffle received the wrong number of bands".into(),
        ));
    }

    let pixels_per_block = match channels {
        2 => 8,
        3 | 4 => 4,
        _ => {
            return Err(PilError::InternalError(
                "portable Merge shuffle requires two to four channels".into(),
            ));
        }
    };
    let vector_pixels = pixels / pixels_per_block * pixels_per_block;
    let mut output = dimensions.alloc_buffer();
    let mut pixel = 0usize;
    let mut vector_blocks = 0u64;
    while pixel < vector_pixels {
        let (block_bytes, block) = merge_wide_block(&bands, pixel, channels)
            .ok_or_else(|| PilError::InternalError("Merge vector shape mismatch".into()))?;
        let start = pixel * channels;
        output[start..start + block_bytes].copy_from_slice(&block[..block_bytes]);
        pixel += pixels_per_block;
        vector_blocks = vector_blocks.saturating_add(1);
    }
    for output_pixel in vector_pixels..pixels {
        let start = output_pixel * channels;
        for (channel, band) in bands.iter().enumerate() {
            output[start + channel] = band[output_pixel];
        }
    }

    crate::compute::record_pipeline_allocation(dimensions.total_bytes());
    crate::compute::record_pipeline_operation_vector_blocks(vector_blocks);
    crate::compute::record_pipeline_operation_scalar_tail(
        ((pixels - vector_pixels) * channels) as u64,
    );
    crate::compute::record_pipeline_operation_path("wide-shuffle");
    let mut output = crate::compute::pool_cpu::ops::composite::helpers::native_bytes::construct(
        width, height, mode, output,
    )?;
    preserve_merged_padding(&mut output, mode, layout, pixels)?;
    Ok(output)
}

fn merge_wide_block(bands: &[&[u8]], pixel: usize, channels: usize) -> Option<(usize, [u8; 16])> {
    let pixels_per_block = match channels {
        2 => 8,
        3 | 4 => 4,
        _ => return None,
    };
    let mut packed = [0u8; 16];
    for (channel, band) in bands.iter().enumerate() {
        let source = band.get(pixel..pixel + pixels_per_block)?;
        let start = channel * pixels_per_block;
        packed[start..start + pixels_per_block].copy_from_slice(source);
    }
    let indices = match channels {
        2 => [0, 8, 1, 9, 2, 10, 3, 11, 4, 12, 5, 13, 6, 14, 7, 15],
        3 => [0, 4, 8, 1, 5, 9, 2, 6, 10, 3, 7, 11, 12, 13, 14, 15],
        4 => [0, 4, 8, 12, 1, 5, 9, 13, 2, 6, 10, 14, 3, 7, 11, 15],
        _ => return None,
    };
    Some((
        pixels_per_block * channels,
        wide::u8x16::new(packed)
            .shuffle(wide::u8x16::new(indices))
            .to_array(),
    ))
}
