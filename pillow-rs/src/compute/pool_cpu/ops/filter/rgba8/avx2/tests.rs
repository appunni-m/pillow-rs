use super::gaussian_blur_radius_seven::Avx2Kernel;

struct ScalarKernel;

impl super::super::helpers::RadiusSevenHorizontalKernel for ScalarKernel {
    const HAS_OPTIMIZED_MIDDLE_BLOCK: bool = false;

    fn blur_middle_block(
        _: &[u8],
        _: &mut [u8],
        _: &mut [u32; 4],
        _: u32,
        _: u32,
        _: usize,
    ) {
        unreachable!("scalar reference must not select an architecture kernel");
    }
}

#[test]
fn rgba_radius_seven_avx2_prefix_blocks_match_scalar_edges_and_tails() {
    if !std::is_x86_feature_detected!("avx2") {
        return;
    }

    const WHOLE_WEIGHT: u32 = 1_116_425;
    const FRACTIONAL_WEIGHT: u32 = 15_420;
    for width in [1, 7, 15, 16, 17, 23, 24, 25, 31, 32, 33, 64, 1024, 1025] {
        for seed in 0..5 {
            let source: Vec<u8> = (0..width * 4)
                .map(|index| {
                    ((index * 73 + width * 31 + seed * 97 + (index >> 2) * 29) % 256) as u8
                })
                .collect();
            let mut expected = vec![0; source.len()];
            let mut actual = vec![0; source.len()];
            super::super::helpers::blur_horizontal_rows::<ScalarKernel>(
                &source,
                &mut expected,
                width,
                1,
                WHOLE_WEIGHT,
                FRACTIONAL_WEIGHT,
            );
            super::super::helpers::blur_horizontal_rows::<Avx2Kernel>(
                &source,
                &mut actual,
                width,
                1,
                WHOLE_WEIGHT,
                FRACTIONAL_WEIGHT,
            );
            assert_eq!(actual, expected, "width={width}, seed={seed}");
        }
    }
}
