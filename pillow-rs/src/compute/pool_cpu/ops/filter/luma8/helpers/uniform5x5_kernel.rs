//! Native L uniform 5x5 filtering with exact portable eight-lane arithmetic.

use wide::{u8x16, u16x8};

/// Apply the exact Pillow uniform 5x5 filter and preserve the copied border.
pub(super) fn filter(
    raw: &[u8],
    out: &mut [u8],
    width: usize,
    height: usize,
) -> Option<(u64, u64)> {
    let pixel_count = width.checked_mul(height)?;
    if raw.len() != pixel_count || out.len() != pixel_count {
        return None;
    }
    if width < 5 || height < 5 {
        return Some((0, 0));
    }
    let blocks = (width - 4) / 8;
    let vector_end = 2 + blocks * 8;
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; width]);
    let fill_row = |source_y: usize, destination: &mut [u16]| {
        let row = &raw[source_y * width..(source_y + 1) * width];
        let mut x = 2;
        while x < vector_end {
            let mut padded = [0u8; 16];
            padded[..12].copy_from_slice(&row[x - 2..x + 10]);
            let window = u8x16::new(padded);
            let tap = |offset: u8| {
                let indices = u8x16::new(std::array::from_fn(|lane| {
                    if lane < 8 { lane as u8 + offset } else { 0 }
                }));
                u16x8::from_u8x16_low(window.swizzle_relaxed(indices))
            };
            let sum = tap(0) + tap(1) + tap(2) + tap(3) + tap(4);
            destination[x..x + 8].copy_from_slice(&sum.to_array());
            x += 8;
        }
        while x < width - 2 {
            destination[x] = row[x - 2..x + 3]
                .iter()
                .map(|&sample| u16::from(sample))
                .sum();
            x += 1;
        }
    };
    for source_y in 0..5 {
        fill_row(source_y, &mut horizontal_rows[source_y]);
    }
    for y in 2..height - 2 {
        let ids = [(y - 2) % 5, (y - 1) % 5, y % 5, (y + 1) % 5, (y + 2) % 5];
        let load = |row_id: usize, x: usize| {
            u16x8::new(horizontal_rows[row_id][x..x + 8].try_into().unwrap())
        };
        let mut x = 2;
        while x < vector_end {
            let sum = load(ids[0], x)
                + load(ids[1], x)
                + load(ids[2], x)
                + load(ids[3], x)
                + load(ids[4], x);
            let quotient = (sum + u16x8::splat(12)).mul_keep_high(u16x8::splat(5243)) >> 1u32;
            let bytes = quotient.to_array().map(|value| value as u8);
            out[y * width + x..y * width + x + 8].copy_from_slice(&bytes);
            x += 8;
        }
        while x < width - 2 {
            let sum = horizontal_rows[ids[0]][x]
                + horizontal_rows[ids[1]][x]
                + horizontal_rows[ids[2]][x]
                + horizontal_rows[ids[3]][x]
                + horizontal_rows[ids[4]][x];
            out[y * width + x] = ((sum + 12) / 25) as u8;
            x += 1;
        }
        let next_y = y + 3;
        if next_y < height {
            fill_row(next_y, &mut horizontal_rows[next_y % 5]);
        }
    }
    Some((
        (blocks * (height - 4)) as u64,
        (((width - 4) % 8) * (height - 4)) as u64,
    ))
}
