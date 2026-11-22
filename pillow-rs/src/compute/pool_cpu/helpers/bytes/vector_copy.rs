//! Shared 128-bit vector copy for native byte spans.

use wide::u8x16;

// VERIFY: source=reviewed@2026-11-15; parity=pending; benchmark=pending
#[inline]
pub(crate) fn copy_native_bytes(source: &[u8], output: &mut [u8]) -> Option<(u64, u64)> {
    if source.len() != output.len() {
        return None;
    }
    if source.is_empty() {
        return Some((0, 0));
    }
    let vector_len = source.len() / 16 * 16;
    let mut vector_blocks = 0u64;
    for offset in (0..vector_len).step_by(16) {
        let block = u8x16::new(<[u8; 16]>::try_from(&source[offset..offset + 16]).ok()?);
        output[offset..offset + 16].copy_from_slice(&block.to_array());
        vector_blocks = vector_blocks.saturating_add(1);
    }
    let tail_len = source.len() - vector_len;
    if tail_len != 0 {
        let mut padded = [0u8; 16];
        padded[..tail_len].copy_from_slice(&source[vector_len..]);
        let block = u8x16::new(padded);
        output[vector_len..].copy_from_slice(&block.to_array()[..tail_len]);
        vector_blocks = vector_blocks.saturating_add(1);
    }
    Some((vector_blocks, 0))
}
