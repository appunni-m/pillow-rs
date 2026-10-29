//! Focused architecture tests for SIMD Duplicate.

#[cfg(all(target_arch = "aarch64", target_feature = "neon"))]
#[test]
fn neon_duplicate_copy_writes_complete_blocks_and_bounded_tails() {
    for length in [0usize, 1, 15, 16, 63, 64, 65, 127, 128, 129] {
        let source = (0..length)
            .map(|index| (index.wrapping_mul(73).wrapping_add(19)) as u8)
            .collect::<Vec<_>>();
        let (output, vector_blocks, scalar_tail) = super::simd_copy_native_bytes(&source);
        let vector_len = length / 64 * 64;

        assert_eq!(output, source, "duplicate copy length {length}");
        assert_eq!(vector_blocks, (vector_len / 16) as u64);
        assert_eq!(scalar_tail, (length - vector_len) as u64);
    }
}
