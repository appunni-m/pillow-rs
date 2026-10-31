//! Native typed-sample Composite kernels for high-depth rasters.

use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::mask::Samples;

pub(super) fn execute_u16(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: Samples<'_>,
    mode: Mode,
    channels: u8,
) -> Result<DynamicImage, PilError> {
    match channels {
        1 => execute_u16_channels::<1>(source, destination, mask, mode),
        2 => execute_u16_channels::<2>(source, destination, mask, mode),
        3 => execute_u16_channels::<3>(source, destination, mask, mode),
        4 => execute_u16_channels::<4>(source, destination, mask, mode),
        _ => Err(PilError::InternalError(
            "Composite u16 strategy has an invalid channel count".into(),
        )),
    }
}

fn execute_u16_channels<const CHANNELS: usize>(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: Samples<'_>,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let source_dims = CheckedDims::new_allow_empty(
        source.width(),
        source.height(),
        (CHANNELS * 2) as u8,
    )?;
    let destination_dims = CheckedDims::new_allow_empty(
        destination.width(),
        destination.height(),
        (CHANNELS * 2) as u8,
    )?;
    let source_bytes = source.as_bytes();
    let destination_bytes = destination.as_bytes();
    if source_bytes.len() != source_dims.total_bytes()
        || destination_bytes.len() != destination_dims.total_bytes()
        || mask.len() != source_dims.total_bytes() / (CHANNELS * 2)
    {
        return Err(PilError::InternalError(
            "Composite u16 storage does not match its dimensions".into(),
        ));
    }
    let mut output = destination_bytes.to_vec();
    blend_u16_samples::<CHANNELS>(
        source_bytes,
        &mut output,
        mask,
        source.width() as usize,
        destination.width() as usize,
        source.width().min(destination.width()) as usize,
        source.height().min(destination.height()) as usize,
    );
    super::output::u16_image(
        destination.width(),
        destination.height(),
        mode,
        output,
        CHANNELS as u8,
    )
}

fn blend_u16_samples<const CHANNELS: usize>(
    source: &[u8],
    destination: &mut [u8],
    mask: Samples<'_>,
    source_width: usize,
    destination_width: usize,
    overlap_width: usize,
    overlap_height: usize,
) {
    for y in 0..overlap_height {
        for x in 0..overlap_width {
            let source_pixel = y * source_width + x;
            let destination_pixel = y * destination_width + x;
            let alpha = u64::from(mask.get(source_pixel));
            let inverse = 255 - alpha;
            for channel in 0..CHANNELS {
                let source_index = (source_pixel * CHANNELS + channel) * 2;
                let destination_index = (destination_pixel * CHANNELS + channel) * 2;
                let source_sample = read_u16(source, source_index);
                let destination_sample = read_u16(destination, destination_index);
                let blended = (u64::from(source_sample) * alpha
                    + u64::from(destination_sample) * inverse
                    + 127)
                    / 255;
                destination[destination_index..destination_index + 2]
                    .copy_from_slice(&(blended as u16).to_ne_bytes());
            }
        }
    }
}

pub(super) fn execute_i32(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: Samples<'_>,
) -> Result<DynamicImage, PilError> {
    let source_dims =
        CheckedDims::new_allow_empty(source.width(), source.height(), 4)?;
    let destination_dims =
        CheckedDims::new_allow_empty(destination.width(), destination.height(), 4)?;
    let source_bytes = source.as_bytes();
    let destination_bytes = destination.as_bytes();
    if source_bytes.len() != source_dims.total_bytes()
        || destination_bytes.len() != destination_dims.total_bytes()
        || mask.len() != source_dims.total_bytes() / 4
    {
        return Err(PilError::InternalError(
            "Composite i32 storage does not match its dimensions".into(),
        ));
    }
    let mut output = destination_bytes.to_vec();
    blend_i32_samples(
        source_bytes,
        &mut output,
        mask,
        source.width() as usize,
        destination.width() as usize,
        source.width().min(destination.width()) as usize,
        source.height().min(destination.height()) as usize,
    );
    super::output::i32_image(destination.width(), destination.height(), output)
}

fn blend_i32_samples(
    source: &[u8],
    destination: &mut [u8],
    mask: Samples<'_>,
    source_width: usize,
    destination_width: usize,
    overlap_width: usize,
    overlap_height: usize,
) {
    for y in 0..overlap_height {
        for x in 0..overlap_width {
            let source_pixel = y * source_width + x;
            let destination_pixel = y * destination_width + x;
            let source_index = source_pixel * 4;
            let destination_index = destination_pixel * 4;
            let alpha = i64::from(mask.get(source_pixel));
            let inverse = 255 - alpha;
            let source_sample = read_i32(source, source_index);
            let destination_sample = read_i32(destination, destination_index);
            let blended = (i64::from(source_sample) * alpha
                + i64::from(destination_sample) * inverse)
                / 255;
            destination[destination_index..destination_index + 4]
                .copy_from_slice(&(blended as i32).to_ne_bytes());
        }
    }
}

pub(super) fn execute_f32(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: Samples<'_>,
    mode: Mode,
    channels: u8,
) -> Result<DynamicImage, PilError> {
    let bytes_per_pixel = channels * 4;
    let source_dims = CheckedDims::new_allow_empty(
        source.width(),
        source.height(),
        bytes_per_pixel,
    )?;
    let destination_dims = CheckedDims::new_allow_empty(
        destination.width(),
        destination.height(),
        bytes_per_pixel,
    )?;
    let source_bytes = source.as_bytes();
    let destination_bytes = destination.as_bytes();
    if source_bytes.len() != source_dims.total_bytes()
        || destination_bytes.len() != destination_dims.total_bytes()
        || mask.len() != source_dims.total_bytes() / usize::from(bytes_per_pixel)
    {
        return Err(PilError::InternalError(
            "Composite f32 storage does not match its dimensions".into(),
        ));
    }
    let mut output = destination_bytes.to_vec();
    blend_f32_samples(
        source_bytes,
        &mut output,
        mask,
        usize::from(channels),
        source.width() as usize,
        destination.width() as usize,
        source.width().min(destination.width()) as usize,
        source.height().min(destination.height()) as usize,
    );
    super::output::f32_image(
        destination.width(),
        destination.height(),
        mode,
        output,
        channels,
    )
}

fn blend_f32_samples(
    source: &[u8],
    destination: &mut [u8],
    mask: Samples<'_>,
    channels: usize,
    source_width: usize,
    destination_width: usize,
    overlap_width: usize,
    overlap_height: usize,
) {
    for y in 0..overlap_height {
        for x in 0..overlap_width {
            let source_pixel = y * source_width + x;
            let destination_pixel = y * destination_width + x;
            let weight = f32::from(mask.get(source_pixel)) / 255.0;
            for channel in 0..channels {
                let source_index = (source_pixel * channels + channel) * 4;
                let destination_index = (destination_pixel * channels + channel) * 4;
                let source_sample = read_f32(source, source_index);
                let destination_sample = read_f32(destination, destination_index);
                let blended = source_sample.mul_add(
                    weight,
                    destination_sample * (1.0 - weight),
                );
                destination[destination_index..destination_index + 4]
                    .copy_from_slice(&blended.to_ne_bytes());
            }
        }
    }
}

fn read_u16(bytes: &[u8], index: usize) -> u16 {
    u16::from_ne_bytes([bytes[index], bytes[index + 1]])
}

fn read_i32(bytes: &[u8], index: usize) -> i32 {
    i32::from_ne_bytes([
        bytes[index],
        bytes[index + 1],
        bytes[index + 2],
        bytes[index + 3],
    ])
}

fn read_f32(bytes: &[u8], index: usize) -> f32 {
    f32::from_ne_bytes([
        bytes[index],
        bytes[index + 1],
        bytes[index + 2],
        bytes[index + 3],
    ])
}
