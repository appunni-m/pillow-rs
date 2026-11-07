//! Native L-mode 5×5 binomial filtering with portable eight-lane vectors.

use wide::{u8x16, u16x8};

fn fill_horizontal_row(
    raw: &[u8],
    width: usize,
    vector_end: usize,
    source_y: usize,
    destination: &mut [u16],
) -> Option<()> {
    let row_start = source_y.checked_mul(width)?;
    let mut x = 2usize;
    while x < vector_end {
        let base = row_start.checked_add(x.checked_sub(2)?)?;
        let window_bytes = raw.get(base..base.checked_add(12)?)?;
        let mut padded = [0u8; 16];
        padded[..12].copy_from_slice(window_bytes);
        let window = u8x16::new(padded);
        let tap = |offset: u8| {
            let indices = u8x16::new(std::array::from_fn(|lane| {
                if lane < 8 { lane as u8 + offset } else { 0 }
            }));
            u16x8::from_u8x16_low(window.shuffle(indices))
        };
        let weighted = tap(0)
            + tap(1) * u16x8::splat(4)
            + tap(2) * u16x8::splat(6)
            + tap(3) * u16x8::splat(4)
            + tap(4);
        destination
            .get_mut(x..x + 8)?
            .copy_from_slice(&weighted.to_array());
        x += 8;
    }
    while x < width - 2 {
        destination[x] = u16::from(raw[row_start + x - 2])
            + u16::from(raw[row_start + x - 1]) * 4
            + u16::from(raw[row_start + x]) * 6
            + u16::from(raw[row_start + x + 1]) * 4
            + u16::from(raw[row_start + x + 2]);
        x += 1;
    }
    Some(())
}

/// Apply Pillow's binomial kernel with rolling horizontal sums and vector rows.
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

    let horizontal_blocks = (width - 4) / 8;
    let vector_end = 2 + horizontal_blocks * 8;
    let mut horizontal_rows: [Vec<u16>; 5] = std::array::from_fn(|_| vec![0; width]);
    for source_y in 0..5 {
        fill_horizontal_row(raw, width, vector_end, source_y, &mut horizontal_rows[source_y])?;
    }
    for y in 2..height - 2 {
        let rows = [
            (y - 2) % 5,
            (y - 1) % 5,
            y % 5,
            (y + 1) % 5,
            (y + 2) % 5,
        ];
        let output_row = y.checked_mul(width)?;
        let mut x = 2usize;
        while x < vector_end {
            let load = |row: usize| -> Option<u16x8> {
                Some(u16x8::new(
                    horizontal_rows[row].get(x..x + 8)?.try_into().ok()?,
                ))
            };
            let weighted = load(rows[0])?
                + load(rows[1])? * u16x8::splat(4)
                + load(rows[2])? * u16x8::splat(6)
                + load(rows[3])? * u16x8::splat(4)
                + load(rows[4])?;
            let rounded = (weighted + u16x8::splat(128)).to_array();
            let bytes = rounded.map(|value| (value >> 8) as u8);
            out.get_mut(output_row + x..output_row + x + 8)?
                .copy_from_slice(&bytes);
            x += 8;
        }
        while x < width - 2 {
            let sum = u32::from(horizontal_rows[rows[0]][x])
                + u32::from(horizontal_rows[rows[1]][x]) * 4
                + u32::from(horizontal_rows[rows[2]][x]) * 6
                + u32::from(horizontal_rows[rows[3]][x]) * 4
                + u32::from(horizontal_rows[rows[4]][x]);
            out[output_row + x] = ((sum + 128) >> 8) as u8;
            x += 1;
        }

        let next_source_y = y + 3;
        if next_source_y < height {
            fill_horizontal_row(
                raw,
                width,
                vector_end,
                next_source_y,
                &mut horizontal_rows[next_source_y % 5],
            )?;
        }
    }

    let interior_height = height - 4;
    Some((
        (horizontal_blocks * interior_height) as u64,
        ((width - 4) % 8 * interior_height) as u64,
    ))
}
