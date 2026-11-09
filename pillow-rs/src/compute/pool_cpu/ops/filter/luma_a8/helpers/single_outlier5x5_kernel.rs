//! Packed-vector native LA kernel for a 5x5 request with one extra tap.

use crate::compute::pool_cpu::helpers::channel::wide::simd_pack_u16x16;
use wide::{u8x16, u16x16};

fn fill_horizontal_row(
    raw: &[u8],
    row_samples: usize,
    width: usize,
    vector_end: usize,
    source_y: usize,
    destination: &mut [u16],
) -> Option<()> {
    let row_start = source_y.checked_mul(row_samples)?;
    let row_end = row_start.checked_add(row_samples)?;
    let row = raw.get(row_start..row_end)?;
    let mut x = 2usize;
    while x < vector_end {
        let load = |tap: usize| -> Option<u16x16> {
            let source_x = x.checked_add(tap)?.checked_sub(2)?;
            let start = row_start.checked_add(source_x.checked_mul(2)?)?;
            let end = start.checked_add(16)?;
            let bytes: [u8; 16] = raw.get(start..end)?.try_into().ok()?;
            Some(u16x16::from(u8x16::new(bytes)))
        };
        let sum = load(0)? + load(1)? + load(2)? + load(3)? + load(4)?;
        let start = x.checked_mul(2)?;
        destination
            .get_mut(start..start.checked_add(16)?)?
            .copy_from_slice(&sum.to_array());
        x += 8;
    }
    while x < width - 2 {
        for channel in 0..2 {
            let mut sum = 0u16;
            for tap in 0..5 {
                sum += u16::from(row[(x + tap - 2) * 2 + channel]);
            }
            destination[x * 2 + channel] = sum;
        }
        x += 1;
    }
    Some(())
}

/// Filter native LA bytes and return vector-block and scalar-tail counts.
///
/// The copied two-pixel border and exact integer rounding match the archived
/// native implementation for the selected single-outlier request.
pub(crate) fn filter_5x5_single_outlier_la_rows(
    raw: &[u8],
    output: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    let row_samples = width.checked_mul(2)?;
    let sample_count = row_samples.checked_mul(height)?;
    if raw.len() != sample_count || output.len() != sample_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }

    let horizontal_blocks = (width - 4) / 8;
    let vector_end = 2 + horizontal_blocks * 8;
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; row_samples]);
    for source_y in 0..5 {
        fill_horizontal_row(
            raw,
            row_samples,
            width,
            vector_end,
            source_y,
            &mut horizontal_rows[source_y],
        )?;
    }
    for y in 2..height - 2 {
        let row_ids = [(y - 2) % 5, (y - 1) % 5, y % 5, (y + 1) % 5, (y + 2) % 5];
        let output_row = y.checked_mul(row_samples)?;
        let extra_row = (y - 2).checked_mul(row_samples)?;
        let mut x = 2usize;
        while x < vector_end {
            let start = x.checked_mul(2)?;
            let load = |row_id: usize| -> Option<u16x16> {
                Some(u16x16::new(
                    horizontal_rows[row_id]
                        .get(start..start.checked_add(16)?)?
                        .try_into()
                        .ok()?,
                ))
            };
            let extra_start = extra_row.checked_add((x + 2).checked_mul(2)?)?;
            let extra_end = extra_start.checked_add(16)?;
            let extra = u16x16::from(u8x16::new(
                raw.get(extra_start..extra_end)?.try_into().ok()?,
            ));
            let sum = load(row_ids[0])?
                + load(row_ids[1])?
                + load(row_ids[2])?
                + load(row_ids[3])?
                + load(row_ids[4])?
                + extra;
            let rounded = sum + u16x16::splat(12);
            let quotient = rounded.mul_keep_high(u16x16::splat(5_243)) >> 1u32;
            let packed = simd_pack_u16x16(quotient.min(u16x16::splat(255))).to_array();
            let output_start = output_row.checked_add(start)?;
            output
                .get_mut(output_start..output_start.checked_add(16)?)?
                .copy_from_slice(&packed);
            x += 8;
        }
        while x < width - 2 {
            let output_start = output_row.checked_add(x.checked_mul(2)?)?;
            let extra_start = extra_row.checked_add((x + 2).checked_mul(2)?)?;
            for channel in 0..2 {
                let sample = x * 2 + channel;
                let sum = u32::from(horizontal_rows[row_ids[0]][sample])
                    + u32::from(horizontal_rows[row_ids[1]][sample])
                    + u32::from(horizontal_rows[row_ids[2]][sample])
                    + u32::from(horizontal_rows[row_ids[3]][sample])
                    + u32::from(horizontal_rows[row_ids[4]][sample])
                    + u32::from(raw[extra_start + channel]);
                output[output_start + channel] = ((sum + 12) / 25).min(255) as u8;
            }
            x += 1;
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(
                raw,
                row_samples,
                width,
                vector_end,
                next_source_y,
                &mut horizontal_rows[next_source_y % 5],
            )?;
        }
    }

    let interior_height = height - 4;
    let vector_blocks = horizontal_blocks.checked_mul(interior_height)?;
    let scalar_tail = ((width - 4) % 8).checked_mul(interior_height)?;
    Some((
        u64::try_from(vector_blocks).ok()?,
        u64::try_from(scalar_tail).ok()?,
    ))
}
