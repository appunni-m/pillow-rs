//! Portable packed-vector implementation of the native LA 5x5 binomial kernel.

use wide::{i16x8, u8x16, u16x16};

#[inline]
fn pack_u16x16(value: u16x16) -> u8x16 {
    let [low, high]: [wide::u16x8; 2] = bytemuck::cast(value);
    let low: i16x8 = bytemuck::cast(low);
    let high: i16x8 = bytemuck::cast(high);
    u8x16::narrow_i16x8(low, high)
}

fn load_vector(samples: &[u16], start: usize) -> Option<u16x16> {
    Some(u16x16::new(
        samples
            .get(start..start.checked_add(16)?)?
            .try_into()
            .ok()?,
    ))
}

fn weighted_horizontal(raw: &[u8], row_start: usize, x: usize) -> Option<u16x16> {
    let load = |tap: usize| {
        let source_x = x.checked_add(tap)?.checked_sub(2)?;
        let start = row_start.checked_add(source_x.checked_mul(2)?)?;
        let bytes: [u8; 16] = raw.get(start..start.checked_add(16)?)?.try_into().ok()?;
        Some(u16x16::from(u8x16::new(bytes)))
    };
    Some(
        load(0)?
            + load(4)?
            + (load(1)? + load(3)?) * u16x16::splat(4)
            + load(2)? * u16x16::splat(6),
    )
}

fn fill_horizontal_row(
    raw: &[u8],
    width: usize,
    source_y: usize,
    destination: &mut [u16],
) -> Option<()> {
    let row_samples = width.checked_mul(2)?;
    let row_start = source_y.checked_mul(row_samples)?;
    let vector_end = 2 + (width - 4) / 8 * 8;
    let mut x = 2usize;
    while x < vector_end {
        let samples = weighted_horizontal(raw, row_start, x)?.to_array();
        destination
            .get_mut(x * 2..x * 2 + 16)?
            .copy_from_slice(&samples);
        x += 8;
    }
    while x < width - 2 {
        let base = row_start + x * 2;
        for channel in 0..2 {
            let sample = base + channel;
            let outer = u16::from(raw[sample - 4]) + u16::from(raw[sample + 4]);
            let inner = u16::from(raw[sample - 2]) + u16::from(raw[sample + 2]);
            destination[x * 2 + channel] = outer + inner * 4 + u16::from(raw[sample]) * 6;
        }
        x += 1;
    }
    Some(())
}

fn weighted_vertical(rows: [&[u16]; 5], start: usize) -> Option<u16x16> {
    let outer = load_vector(rows[0], start)? + load_vector(rows[4], start)?;
    let inner = load_vector(rows[1], start)? + load_vector(rows[3], start)?;
    Some(outer + inner * u16x16::splat(4) + load_vector(rows[2], start)? * u16x16::splat(6))
}

fn write_vertical_row(
    rows: [&[u16]; 5],
    output: &mut [u8],
    width: usize,
    row_samples: usize,
    y: usize,
) -> Option<(usize, usize)> {
    let vector_end = 2 + (width - 4) / 8 * 8;
    let output_row = y.checked_mul(row_samples)?;
    let mut x = 2usize;
    while x < vector_end {
        let start = x.checked_mul(2)?;
        let packed =
            pack_u16x16((weighted_vertical(rows, start)? + u16x16::splat(128)) >> 8u32).to_array();
        output
            .get_mut(output_row + start..output_row + start + 16)?
            .copy_from_slice(&packed);
        x += 8;
    }
    let scalar_start = x;
    while x < width - 2 {
        let base = x * 2;
        for channel in 0..2 {
            let sample = base + channel;
            let outer = u32::from(rows[0][sample]) + u32::from(rows[4][sample]);
            let inner = u32::from(rows[1][sample]) + u32::from(rows[3][sample]);
            let center = u32::from(rows[2][sample]);
            output[output_row + sample] = ((outer + inner * 4 + center * 6 + 128) >> 8) as u8;
        }
        x += 1;
    }
    Some(((vector_end - 2) / 8, x - scalar_start))
}

/// Filters native LA bytes and returns vector-block and scalar-tail counts.
pub(crate) fn filter_5x5_binomial_la_rows(
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
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; row_samples]);
    for source_y in 0..5 {
        fill_horizontal_row(raw, width, source_y, &mut horizontal_rows[source_y])?;
    }
    let (mut vector_blocks, mut scalar_tail) = (0usize, 0usize);
    for y in 2..height - 2 {
        let rows = [
            &horizontal_rows[(y - 2) % 5][..],
            &horizontal_rows[(y - 1) % 5][..],
            &horizontal_rows[y % 5][..],
            &horizontal_rows[(y + 1) % 5][..],
            &horizontal_rows[(y + 2) % 5][..],
        ];
        let (row_vectors, row_tail) = write_vertical_row(rows, output, width, row_samples, y)?;
        vector_blocks += row_vectors;
        scalar_tail += row_tail;
        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(
                raw,
                width,
                next_source_y,
                &mut horizontal_rows[next_source_y % 5],
            )?;
        }
    }
    Some((vector_blocks as u64, scalar_tail as u64))
}
