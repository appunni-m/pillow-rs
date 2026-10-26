use super::transpose_resize_intermediate;

#[test]
fn tiled_rgb_transpose_matches_reference_across_partial_tiles() {
    for (source_rows, output_width) in [(1u32, 1u32), (31, 33), (33, 31), (65, 67)] {
        let source = (0..source_rows as usize * output_width as usize * 3)
            .map(|index| (index.wrapping_mul(73).wrapping_add(index / 7 * 19)) as u8)
            .collect::<Vec<_>>();
        let mut expected = vec![0; source.len()];
        let source_stride = output_width as usize * 3;
        let destination_stride = source_rows as usize * 3;
        for x in 0..output_width as usize {
            for y in 0..source_rows as usize {
                let source_start = y * source_stride + x * 3;
                let destination_start = x * destination_stride + y * 3;
                expected[destination_start..destination_start + 3]
                    .copy_from_slice(&source[source_start..source_start + 3]);
            }
        }

        assert_eq!(
            transpose_resize_intermediate(&source, source_rows, output_width, 3),
            expected,
            "source_rows={source_rows}, output_width={output_width}"
        );
    }
}
