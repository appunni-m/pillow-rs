//! Existing portable-vector row fill reused by exact-mode gradient strategies.

use crate::error::PilError;
use wide::u8x16;

pub(crate) fn fill_pattern(row: &mut [u8], pattern: &[u8]) -> Result<(u64, u64), PilError> {
    if pattern.is_empty() || pattern.len() > 16 || 16 % pattern.len() != 0 {
        return Err(PilError::InternalError(
            "LinearGradient vector pattern must divide a 16-byte block".into(),
        ));
    }

    let mut lanes = [0u8; 16];
    for (index, lane) in lanes.iter_mut().enumerate() {
        *lane = pattern[index % pattern.len()];
    }
    let block = u8x16::new(lanes).to_array();
    let vector_len = row.len() / block.len() * block.len();
    let mut vector_blocks = 0u64;
    for chunk in row[..vector_len].chunks_exact_mut(block.len()) {
        chunk.copy_from_slice(&block);
        vector_blocks = vector_blocks.saturating_add(1);
    }

    let tail = &mut row[vector_len..];
    for (index, byte) in tail.iter_mut().enumerate() {
        *byte = pattern[index % pattern.len()];
    }
    Ok((vector_blocks, tail.len() as u64))
}
