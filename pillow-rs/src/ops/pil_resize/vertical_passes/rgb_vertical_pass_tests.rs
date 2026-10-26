use super::{
    filter_from_resample, precompute_coeffs_boxed, vertical_pass_col, vertical_pass_rgb_row,
};
use crate::pipeline::ResampleFilter;

#[test]
fn rgb_row_kernel_matches_column_reference_for_boxed_filters_and_edges() {
    for filter in [
        ResampleFilter::Bilinear,
        ResampleFilter::Bicubic,
        ResampleFilter::Lanczos,
        ResampleFilter::Box,
        ResampleFilter::Hamming,
    ] {
        let (kernel, support) = filter_from_resample(filter);
        for (source_rows, output_height, output_width) in
            [(7u32, 3u32, 1u32), (19, 11, 7), (37, 23, 31)]
        {
            let coeffs = precompute_coeffs_boxed(
                output_height,
                source_rows,
                0.0,
                f64::from(source_rows),
                kernel,
                support,
            );
            let source = (0..source_rows as usize * output_width as usize * 3)
                .map(|index| {
                    index
                        .wrapping_mul(71)
                        .wrapping_add(index / (output_width as usize * 3) * 43)
                        as u8
                })
                .collect::<Vec<_>>();
            for y in 0..output_height as usize {
                let mut expected = vec![0u8; output_width as usize * 3];
                for x in 0..output_width {
                    let pixel =
                        vertical_pass_col(&source, source_rows, x, output_width, 3, &coeffs, y);
                    let start = x as usize * 3;
                    expected[start..start + 3].copy_from_slice(&pixel[..3]);
                }
                let mut actual = vec![0u8; output_width as usize * 3];
                vertical_pass_rgb_row(&source, output_width, &coeffs, y, &mut actual);
                assert_eq!(
                    actual, expected,
                    "{filter:?} {source_rows}x{output_width} row {y}"
                );
            }
        }
    }
}
