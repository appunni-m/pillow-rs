//! Original nonblending masked native store, with layout fixed before execution.

use wide::u8x16;

fn source_byte<const CHANNELS: usize>(fill: (u8, u8, u8, u8), byte: usize) -> u8 {
    match CHANNELS {
        1 => fill.0,
        2 => match byte % 2 { 0 => fill.0, _ => fill.3 },
        3 => match byte % 3 { 0 => fill.0, 1 => fill.1, _ => fill.2 },
        4 => match byte % 4 { 0 => fill.0, 1 => fill.1, 2 => fill.2, _ => fill.3 },
        _ => 0,
    }
}

/// Bounds and global byte phase are the same as the original draw/PutPixel store.
#[inline]
pub(crate) fn store<const CHANNELS: usize>(
    row: &mut [u8], block_start: usize, target_start: usize, target_end: usize,
    fill: (u8, u8, u8, u8),
) {
    let input = <[u8; 16]>::try_from(&row[block_start..block_start + 16])
        .expect("draw line vector block has 16 bytes");
    let source = u8x16::new(std::array::from_fn(|lane| {
        source_byte::<CHANNELS>(fill, block_start + lane)
    }));
    let mask = u8x16::new(std::array::from_fn(|lane| {
        let position = block_start + lane;
        if (target_start..target_end).contains(&position) { u8::MAX } else { 0 }
    }));
    let input = u8x16::new(input);
    let output = (source & mask) | (input & (u8x16::splat(u8::MAX) ^ mask));
    row[block_start..block_start + 16].copy_from_slice(&output.to_array());
}
