//! Native-byte Brightness kernel shared by exact mode architecture strategies.

use super::native;
use crate::checked_dims::CheckedDims;
use crate::error::PilError;
use crate::raster::{ImageBuffer, Pixel};
use wide::u8x16;

pub(crate) fn execute_image<P>(
    source: &ImageBuffer<P, Vec<u8>>,
    channels: u8,
    active_channels: u8,
    right_shift_factors: bool,
    factor: f64,
) -> Result<ImageBuffer<P, Vec<u8>>, PilError>
where
    P: Pixel<Subpixel = u8>,
{
    let output = execute(
        source.as_raw(),
        source.width(),
        source.height(),
        channels,
        active_channels,
        right_shift_factors,
        factor,
    )?;
    ImageBuffer::from_raw(source.width(), source.height(), output).ok_or_else(|| {
        PilError::InternalError("Brightness vector output did not match its admitted shape".into())
    })
}

pub(crate) fn execute(
    source: &[u8],
    width: u32,
    height: u32,
    channels: u8,
    active_channels: u8,
    right_shift_factors: bool,
    factor: f64,
) -> Result<Vec<u8>, PilError> {
    if !(1..=4).contains(&channels) || active_channels == 0 || active_channels > channels {
        return Err(PilError::InternalError(
            "Brightness vector strategy received an invalid native layout".into(),
        ));
    }
    let dimensions = CheckedDims::new_allow_empty(width, height, channels)?;
    if source.is_empty() || source.len() != dimensions.total_bytes() || !factor.is_finite() {
        return Err(PilError::InternalError(
            "Brightness vector input did not match its admitted layout".into(),
        ));
    }
    let alpha = factor as f32;
    if alpha == 1.0 {
        crate::compute::record_pipeline_operation_path("native-copy");
        return Ok(source.to_vec());
    }

    let mut output = vec![0; source.len()];
    let (active, inactive) = active_masks(usize::from(channels), usize::from(active_channels));
    let active = u8x16::new(active);
    let inactive = u8x16::new(inactive);
    let shift = if right_shift_factors {
        match alpha {
            0.5 => Some(1),
            0.25 => Some(2),
            0.125 => Some(3),
            _ => None,
        }
    } else {
        None
    };
    if let Some(shift) = shift {
        apply(source, &mut output, active, inactive, |input| input >> shift);
    } else {
        let tables = lut_tables(alpha);
        apply(source, &mut output, active, inactive, |input| {
            lut_chunk(input, &tables)
        });
    }
    let blocks = source.len().div_ceil(16);
    crate::compute::record_pipeline_operation_vector_blocks(
        u64::try_from(blocks).unwrap_or(u64::MAX),
    );
    crate::compute::record_pipeline_operation_path("vector");
    Ok(output)
}

fn active_masks(channels: usize, active_channels: usize) -> ([u8; 16], [u8; 16]) {
    let active = std::array::from_fn(|index| {
        if index % channels < active_channels {
            u8::MAX
        } else {
            0
        }
    });
    let inactive = active.map(|value| u8::MAX - value);
    (active, inactive)
}

/// Applies flat 16-byte blocks with the mode strategy's repeated channel mask.
/// Partial-channel layouts are LA (2-byte pixels) and RGBA (4-byte pixels),
/// whose strides divide a vector block; three-byte modes activate every lane.
fn apply<F>(
    source: &[u8],
    output: &mut [u8],
    active: u8x16,
    inactive: u8x16,
    transform: F,
) where
    F: Fn(u8x16) -> u8x16,
{
    let (source_blocks, source_tail) = source.as_chunks::<16>();
    let (output_blocks, output_tail) = output.as_chunks_mut::<16>();
    for (source, output) in source_blocks.iter().zip(output_blocks) {
        let input = u8x16::new(*source);
        *output = ((transform(input) & active) | (input & inactive)).to_array();
    }
    if !source_tail.is_empty() {
        let mut padded = [0; 16];
        padded[..source_tail.len()].copy_from_slice(source_tail);
        let input = u8x16::new(padded);
        let transformed = ((transform(input) & active) | (input & inactive)).to_array();
        output_tail.copy_from_slice(&transformed[..source_tail.len()]);
    }
}

fn lut_tables(alpha: f32) -> [u8x16; 16] {
    std::array::from_fn(|table| {
        u8x16::new(std::array::from_fn(|lane| {
            let value = (table * 16 + lane) as u8;
            native::blend_u8(0, value, alpha)
        }))
    })
}

#[inline]
fn lut_chunk(input: u8x16, tables: &[u8x16; 16]) -> u8x16 {
    let low = input & u8x16::splat(0x0f);
    let high: u8x16 = input >> 4u32;
    let mut output = tables[0].swizzle_relaxed(low);
    for (index, table) in tables.iter().enumerate().skip(1) {
        let selected = high
            .simd_eq(u8x16::splat(index as u8))
            .select(table.swizzle_relaxed(low), output);
        output = selected;
    }
    output
}
