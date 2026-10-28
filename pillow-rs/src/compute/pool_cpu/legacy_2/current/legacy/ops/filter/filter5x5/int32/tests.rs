#[cfg(test)]
mod i32_filter5x5_uniform_tests {
    use super::super::{execute_filter5x5, uniform_i32_filter5x5_is_identity};
    use crate::raster::{DynamicImage, RgbaImage};

    #[test]
    fn uniform_binomial_filter_returns_the_exact_input() {
        let (width, height, sample) = (32, 24, 37i32);
        let pixel = sample.to_le_bytes();
        let raw = pixel.repeat(width * height);
        let kernel = [
            1.0, 4.0, 6.0, 4.0, 1.0, 4.0, 16.0, 24.0, 16.0, 4.0, 6.0, 24.0, 36.0, 24.0, 6.0, 4.0,
            16.0, 24.0, 16.0, 4.0, 1.0, 4.0, 6.0, 4.0, 1.0,
        ];
        let normalized = std::array::from_fn(|index| kernel[index] / 256.0);
        assert!(uniform_i32_filter5x5_is_identity(
            &raw,
            width,
            height,
            &normalized,
            0.5,
        ));

        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width as u32, height as u32, raw.clone()).unwrap(),
        );
        let result = execute_filter5x5(&image, &kernel, 256.0, 0.0, Some("I")).unwrap();
        assert_eq!(result.as_bytes(), raw);
    }

    #[test]
    fn uniformity_proof_rejects_nonidentity_and_nonuniform_inputs() {
        let (width, height, sample) = (7, 6, 11i32);
        let pixel = sample.to_le_bytes();
        let raw = pixel.repeat(width * height);
        let mut identity_kernel = [0.0; 25];
        identity_kernel[12] = 1.0;
        assert!(uniform_i32_filter5x5_is_identity(
            &raw,
            width,
            height,
            &identity_kernel,
            0.5,
        ));

        let mut nonidentity_kernel = identity_kernel;
        nonidentity_kernel[12] = 2.0;
        assert!(!uniform_i32_filter5x5_is_identity(
            &raw,
            width,
            height,
            &nonidentity_kernel,
            0.5,
        ));

        let mut varied = raw;
        varied[4 * (width + 3)..4 * (width + 3) + 4].copy_from_slice(&12i32.to_le_bytes());
        assert!(!uniform_i32_filter5x5_is_identity(
            &varied,
            width,
            height,
            &identity_kernel,
            0.5,
        ));
    }
}

#[cfg(all(test, not(feature = "parallel")))]
mod i32_filter5x5_sliding_window_tests {
    use super::super::PortableF32Fma;
    use super::super::{
        DynamicImage, filter_5x5_i32, filter_5x5_i32_bounded_binomial_from_samples,
        filter_5x5_i32_f32_binomial_scaled_rows_ring_with_fma, filter_5x5_i32_f32_ring,
        i32_filter5x5_binomial_kernel_is_applicable,
        i32_filter5x5_bounded_binomial_kernel_is_applicable, pillow_kernel_row_5,
    };

    fn scalar_reference_row(
        raw: &[u8],
        row: &mut [u8],
        y: i32,
        width: i32,
        height: i32,
        kernel: &[f32; 25],
        offset: f32,
    ) {
        if y < 2 || y >= height - 2 || width < 5 {
            return;
        }
        for x in 2..width - 2 {
            let mut value = offset + 0.5;
            for (row_index, dy) in [2, 1, 0, -1, -2].into_iter().enumerate() {
                let samples = std::array::from_fn(|tap| {
                    let pixel = ((y + dy) * width + x + tap as i32 - 2) as usize;
                    i32::from_le_bytes([
                        raw[pixel * 4],
                        raw[pixel * 4 + 1],
                        raw[pixel * 4 + 2],
                        raw[pixel * 4 + 3],
                    ]) as f32
                });
                value += pillow_kernel_row_5(samples, &kernel[row_index * 5..row_index * 5 + 5]);
            }
            let result = if value >= 0.0 { value as i32 } else { 0 };
            let output = x as usize * 4;
            row[output..output + 4].copy_from_slice(&result.to_le_bytes());
        }
    }

    #[test]
    fn sliding_windows_preserve_scalar_fma_results_for_signed_i32_samples() {
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut values = vec![i32::MIN, i32::MAX, -16_777_217, 16_777_217, -1, 0, 1];
        for _ in values.len()..19 * 13 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            values.push(state as u32 as i32);
        }

        let kernel = std::array::from_fn(|index| ((index as i32 * 17 % 23) - 11) as f32 / 16.0);
        for (width, height) in [(5usize, 5usize), (6, 7), (19, 13)] {
            let mut raw = Vec::with_capacity(width * height * 4);
            for index in 0..width * height {
                raw.extend_from_slice(&values[index].to_le_bytes());
            }
            for offset in [0.0f32, 0.5, -17.25, 93.125] {
                let row_stride = width * 4;
                let mut expected = raw.clone();
                for y in 0..height {
                    let row_start = y * row_stride;
                    scalar_reference_row(
                        &raw,
                        &mut expected[row_start..row_start + row_stride],
                        y as i32,
                        width as i32,
                        height as i32,
                        &kernel,
                        offset,
                    );
                }
                let actual = filter_5x5_i32_f32_ring(&raw, width, height, &kernel, offset).unwrap();
                for pixel in 0..width * height {
                    let start = pixel * 4;
                    assert_eq!(
                        &actual[start..start + 4],
                        &expected[start..start + 4],
                        "sample mismatch at ({}, {}) for offset {offset}",
                        pixel % width,
                        pixel / width,
                    );
                }
            }
        }
    }

    #[test]
    fn bounded_binomial_integer_ring_matches_f32_at_minimum_and_odd_dimensions() {
        let weights = [1i32, 4, 6, 4, 1];
        let kernel =
            std::array::from_fn(|index| (weights[index / 5] * weights[index % 5]) as f32 / 256.0);
        let values: [i32; 7] = [-65_535, -32_768, -1, 0, 1, 32_767, 65_535];

        for (width, height) in [(5usize, 5usize), (6, 6), (7, 8), (8, 5)] {
            let samples = (0..width * height)
                .map(|index| values[(index * 5 + index / width) % values.len()])
                .collect::<Vec<_>>();
            let raw = samples
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect::<Vec<_>>();
            assert!(i32_filter5x5_bounded_binomial_kernel_is_applicable(
                samples.len(),
                width,
                height,
                &kernel,
                0.0,
            ));

            let actual =
                filter_5x5_i32_bounded_binomial_from_samples(&raw, &samples, width, height)
                    .unwrap();
            let reference = filter_5x5_i32_f32_ring(&raw, width, height, &kernel, 0.0).unwrap();
            assert_eq!(
                actual, reference,
                "bounded kernel mismatch at {width}×{height}"
            );

            for out_of_bound_index in [0, samples.len() / 2, samples.len() - 1] {
                let mut out_of_bound = samples.clone();
                out_of_bound[out_of_bound_index] = 65_536;
                let out_of_bound_raw = out_of_bound
                    .iter()
                    .flat_map(|sample| sample.to_le_bytes())
                    .collect::<Vec<_>>();
                assert!(i32_filter5x5_bounded_binomial_kernel_is_applicable(
                    out_of_bound.len(),
                    width,
                    height,
                    &kernel,
                    0.0,
                ));
                assert!(
                    filter_5x5_i32_bounded_binomial_from_samples(
                        &out_of_bound_raw,
                        &out_of_bound,
                        width,
                        height,
                    )
                    .is_none()
                );

                let image = DynamicImage::ImageRgba8(
                    crate::raster::RgbaImage::from_raw(
                        width as u32,
                        height as u32,
                        out_of_bound_raw.clone(),
                    )
                    .unwrap(),
                );
                let actual = filter_5x5_i32(&image, &kernel, 1.0, 0.0).unwrap();
                let DynamicImage::ImageRgba8(actual) = actual else {
                    panic!("I-mode filter must preserve its scalar carrier");
                };
                let reference =
                    filter_5x5_i32_f32_ring(&out_of_bound_raw, width, height, &kernel, 0.0)
                        .unwrap();
                assert_eq!(actual.as_raw(), &reference);
            }
        }
    }

    #[test]
    fn binomial_scaled_rows_ring_matches_pillow_fma_for_full_i32_range() {
        let weights = [1i32, 4, 6, 4, 1];
        let kernel =
            std::array::from_fn(|index| (weights[index / 5] * weights[index % 5]) as f32 / 256.0);
        assert!(i32_filter5x5_binomial_kernel_is_applicable(&kernel));

        let mut state = 0x6a09_e667_f3bc_c909u64;
        let mut values = vec![i32::MIN, i32::MAX, -999_997, 999_996, -1, 0, 1];
        for _ in values.len()..19 * 13 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            values.push(state as u32 as i32);
        }

        for (width, height) in [(5usize, 5usize), (6, 7), (19, 13)] {
            let samples = (0..width * height)
                .map(|index| values[index])
                .collect::<Vec<_>>();
            let raw = samples
                .iter()
                .flat_map(|sample| sample.to_le_bytes())
                .collect::<Vec<_>>();
            for offset in [0.0f32, 0.5, -17.25] {
                let mut expected = raw.clone();
                let row_stride = width * 4;
                for y in 0..height {
                    let row_start = y * row_stride;
                    scalar_reference_row(
                        &raw,
                        &mut expected[row_start..row_start + row_stride],
                        y as i32,
                        width as i32,
                        height as i32,
                        &kernel,
                        offset,
                    );
                }

                let actual = filter_5x5_i32_f32_binomial_scaled_rows_ring_with_fma(
                    &raw,
                    &samples,
                    width,
                    height,
                    &kernel,
                    offset,
                    &PortableF32Fma,
                )
                .unwrap();
                assert_eq!(
                    actual, expected,
                    "FMA mismatch at {width}×{height}, offset {offset}"
                );

                let image = DynamicImage::ImageRgba8(
                    crate::raster::RgbaImage::from_raw(width as u32, height as u32, raw.clone())
                        .unwrap(),
                );
                let DynamicImage::ImageRgba8(actual) = filter_5x5_i32(
                    &image,
                    &kernel.map(|coefficient| coefficient * 256.0),
                    256.0,
                    offset,
                )
                .unwrap() else {
                    panic!("I-mode filter must preserve its scalar carrier");
                };
                assert_eq!(
                    actual.as_raw(),
                    &expected,
                    "dispatch mismatch at {width}×{height}, offset {offset}"
                );
            }
        }
    }

    #[test]
    fn unaligned_i32_source_preserves_filter_output() {
        let width = 9usize;
        let height = 7usize;
        let values = [i32::MIN, -16_777_217, -1, 0, 1, 65_535, 65_536, i32::MAX];
        let mut raw = Vec::with_capacity(width * height * 4);
        for index in 0..width * height {
            raw.extend_from_slice(&values[index % values.len()].to_le_bytes());
        }
        let mut storage = vec![0; raw.len() + std::mem::align_of::<i32>()];
        let base = storage.as_ptr() as usize;
        let offset = (0..std::mem::align_of::<i32>())
            .find(|offset| (base + offset) % std::mem::align_of::<i32>() != 0)
            .expect("an unaligned byte offset must exist");
        storage[offset..offset + raw.len()].copy_from_slice(&raw);
        let unaligned = &storage[offset..offset + raw.len()];

        let kernel = std::array::from_fn(|index| ((index as i32 * 13 % 19) - 9) as f32 / 8.0);
        let offset = -3.25f32;
        let row_stride = width * 4;
        let mut expected = raw.clone();
        for y in 0..height {
            let row_start = y * row_stride;
            scalar_reference_row(
                &raw,
                &mut expected[row_start..row_start + row_stride],
                y as i32,
                width as i32,
                height as i32,
                &kernel,
                offset,
            );
        }
        let actual = filter_5x5_i32_f32_ring(unaligned, width, height, &kernel, offset).unwrap();
        assert_eq!(actual, expected);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn runtime_x86_f32_fma_matches_portable_fused_operation() {
        use super::super::{F32MulAdd, X86F32FmaToken, i32_filter_f32_fma_is_safe};

        let Some(x86_fma) = X86F32FmaToken::detect() else {
            return;
        };
        let portable = PortableF32Fma;
        for (sample, weight, accumulator) in [
            (0.1f32, 0.2, 0.3),
            (1.0 + f32::EPSILON, 1.0 - f32::EPSILON, -1.0),
            (f32::MIN_POSITIVE, 1.0, -0.0),
            (-0.0, 1.0, -0.0),
            (f32::MAX * 0.125, 1.5, -f32::MAX * 0.125),
        ] {
            assert_eq!(
                x86_fma.mul_add(sample, weight, accumulator).to_bits(),
                portable.mul_add(sample, weight, accumulator).to_bits(),
                "FMA result differs for {sample:?} * {weight:?} + {accumulator:?}",
            );
        }

        let factors = [1.0f32, 4.0, 6.0, 4.0, 1.0];
        let kernel = std::array::from_fn(|index| factors[index % 5] * factors[index / 5] / 256.0);
        assert!(i32_filter_f32_fma_is_safe(&kernel, 0.0));
        assert!(i32_filter_f32_fma_is_safe(&kernel, -0.0));

        let mut nonfinite = kernel;
        nonfinite[7] = f32::NAN;
        assert!(!i32_filter_f32_fma_is_safe(&nonfinite, 0.0));
        assert!(!i32_filter_f32_fma_is_safe(&kernel, f32::INFINITY));

        let mut excessive = [0.0f32; 25];
        excessive[12] = f32::MAX;
        assert!(!i32_filter_f32_fma_is_safe(&excessive, 0.0));
    }
}

#[cfg(all(
    test,
    not(feature = "parallel"),
    any(
        all(target_arch = "aarch64", target_feature = "neon"),
        all(target_arch = "x86_64", target_feature = "sse2")
    )
))]
mod native_i32_vector_filter_tests {
    use super::super::filter_5x5_i32_row;

    #[test]
    fn vector_i32_kernel_matches_ordered_f32_across_width_tails_and_borders() {
        let factors = [1i32, 4, 6, 4, 1];
        let kernel =
            std::array::from_fn(|index| (factors[index / 5] * factors[index % 5]) as f32 / 256.0);
        let patterns = [
            i32::MIN,
            i32::MAX,
            -16_777_217,
            -65_536,
            -1,
            0,
            1,
            65_535,
            65_536,
            16_777_219,
        ];

        for (width, height) in [(5usize, 5usize), (13, 7), (19, 11), (20, 13), (31, 17)] {
            let raw = (0..width * height)
                .flat_map(|index| {
                    patterns[(index * 7 + index / width * 3) % patterns.len()].to_le_bytes()
                })
                .collect::<Vec<_>>();
            for offset in [0.0f32, 0.5, -17.25] {
                let mut expected = raw.clone();
                let row_stride = width * 4;
                for y in 0..height {
                    let row_start = y * row_stride;
                    filter_5x5_i32_row(
                        &raw,
                        &mut expected[row_start..row_start + row_stride],
                        y as i32,
                        width as i32,
                        height as i32,
                        &kernel,
                        offset,
                    );
                }

                let mut actual = raw.clone();
                let (vector_blocks, scalar_tail) =
                    crate::compute::pool_simd::ops::native_filter::native_filter_5x5_i32_rows(
                        &raw,
                        &mut actual,
                        width,
                        height,
                        &kernel,
                        offset + 0.5,
                    )
                    .expect("valid native I-mode vector dimensions");
                assert!(vector_blocks > 0);
                assert_eq!(scalar_tail, 0);
                assert_eq!(actual, expected, "I-mode {width}x{height}, offset {offset}");
            }
        }
    }
}
