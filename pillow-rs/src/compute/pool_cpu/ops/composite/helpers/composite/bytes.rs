//! Native byte-sample Composite kernel.

use super::mask::Samples;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(super) fn execute(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: Samples<'_>,
    mode: Mode,
    channels: u8,
) -> Result<DynamicImage, PilError> {
    match channels {
        1 => execute_channels::<1>(source, destination, mask, mode),
        2 => execute_channels::<2>(source, destination, mask, mode),
        3 => execute_channels::<3>(source, destination, mask, mode),
        4 => execute_channels::<4>(source, destination, mask, mode),
        _ => Err(PilError::InternalError(
            "Composite byte strategy has an invalid channel count".into(),
        )),
    }
}

fn execute_channels<const CHANNELS: usize>(
    source: &DynamicImage,
    destination: &DynamicImage,
    mask: Samples<'_>,
    mode: Mode,
) -> Result<DynamicImage, PilError> {
    let (source_width, source_height) = (source.width(), source.height());
    let (destination_width, destination_height) = (destination.width(), destination.height());
    let source_dims = CheckedDims::new_allow_empty(source_width, source_height, CHANNELS as u8)?;
    let destination_dims =
        CheckedDims::new_allow_empty(destination_width, destination_height, CHANNELS as u8)?;
    if source.as_bytes().len() != source_dims.total_bytes()
        || destination.as_bytes().len() != destination_dims.total_bytes()
        || mask.len() != source_dims.total_bytes() / CHANNELS
    {
        return Err(PilError::InternalError(
            "Composite byte storage does not match its dimensions".into(),
        ));
    }
    let mut output = destination.as_bytes().to_vec();
    let overlap_width = source_width.min(destination_width) as usize;
    let overlap_height = source_height.min(destination_height) as usize;
    let source_width = source_width as usize;
    let destination_width = destination_width as usize;

    for y in 0..overlap_height {
        let source_row = y * source_width;
        let destination_row = y * destination_width;
        for x in 0..overlap_width {
            let source_pixel = source_row + x;
            let destination_pixel = destination_row + x;
            let alpha = mask.get(source_pixel);
            for channel in 0..CHANNELS {
                let source_index = source_pixel * CHANNELS + channel;
                let destination_index = destination_pixel * CHANNELS + channel;
                output[destination_index] = blend(
                    source.as_bytes()[source_index],
                    output[destination_index],
                    alpha,
                );
            }
        }
    }

    crate::compute::pool_cpu::ops::composite::helpers::native_bytes::construct(
        destination_width as u32,
        destination_height,
        mode,
        output,
    )
}

#[inline]
fn blend(source: u8, destination: u8, mask: u8) -> u8 {
    let mask = u16::from(mask);
    let inverse = 255 - mask;
    ((u16::from(source) * mask + u16::from(destination) * inverse + 127) / 255) as u8
}
