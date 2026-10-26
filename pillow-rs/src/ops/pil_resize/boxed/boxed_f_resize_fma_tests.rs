use super::{
    PortableFma, X86FmaToken, f_resize_coefficients_allow_finite_fma,
    f_resize_samples_allow_finite_fma, pil_resize_f_boxed, pil_resize_f_boxed_with_fma,
    precompute_coeffs_f64_boxed,
};
use crate::pipeline::ResampleFilter;
use crate::raster::{DynamicImage, RgbaImage};

#[test]
fn runtime_x86_fma_boxed_resize_matches_portable_fused_operations() {
    let Some(x86_fma) = X86FmaToken::detect() else {
        return;
    };
    let (source_width, source_height) = (96u32, 48u32);
    let samples = (0..source_width as usize * source_height as usize)
        .map(|index| (((index * 37 % 257) as i32 - 128) as f32) / 31.0)
        .collect::<Vec<_>>();
    let bytes = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(source_width, source_height, bytes).expect("packed F samples"),
    );
    let bounds = (0.125, 0.25, 95.625, 47.5);
    let actual = pil_resize_f_boxed_with_fma::<_, true>(
        &image,
        17,
        9,
        bounds.0,
        bounds.1,
        bounds.2,
        bounds.3,
        ResampleFilter::Lanczos,
        &x86_fma,
    );
    let selected = pil_resize_f_boxed(
        &image,
        17,
        9,
        bounds.0,
        bounds.1,
        bounds.2,
        bounds.3,
        ResampleFilter::Lanczos,
    );
    let portable = pil_resize_f_boxed_with_fma::<_, true>(
        &image,
        17,
        9,
        bounds.0,
        bounds.1,
        bounds.2,
        bounds.3,
        ResampleFilter::Lanczos,
        &PortableFma,
    );
    assert_eq!(selected.as_bytes(), actual.as_bytes());
    assert_eq!(actual.as_bytes(), portable.as_bytes());
}

#[test]
fn finite_x86_fma_boxed_resize_matches_portable_and_rejects_nonfinite_samples() {
    let Some(x86_fma) = X86FmaToken::detect() else {
        return;
    };
    let (source_width, source_height) = (96u32, 48u32);
    let bounds = (0.125, 0.25, 95.625, 47.5);
    let finite = (0..source_width as usize * source_height as usize)
        .map(|index| (((index * 37 % 257) as i32 - 128) as f32) / 31.0)
        .collect::<Vec<_>>();
    let mut overflow_edges = vec![f32::MAX; source_width as usize * source_height as usize];
    for (index, sample) in overflow_edges.iter_mut().enumerate() {
        if index % 2 == 0 {
            *sample = -f32::MAX;
        }
    }
    let mut nonfinite = finite.clone();
    nonfinite[7] = f32::from_bits(0x7fc1_2345);
    nonfinite[19] = f32::INFINITY;
    nonfinite[41] = f32::NEG_INFINITY;

    for (filter, samples) in [
        (ResampleFilter::Bicubic, finite.as_slice()),
        (ResampleFilter::Bicubic, overflow_edges.as_slice()),
        (ResampleFilter::Lanczos, finite.as_slice()),
        (ResampleFilter::Lanczos, nonfinite.as_slice()),
    ] {
        let source_bytes = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(source_width, source_height, source_bytes)
                .expect("packed F sample source shape must match"),
        );
        let horizontal = precompute_coeffs_f64_boxed(17, source_width, bounds.0, bounds.2, filter);
        let vertical = precompute_coeffs_f64_boxed(9, source_height, bounds.1, bounds.3, filter);
        let expected_finite_samples = samples.iter().all(|sample| sample.is_finite());
        assert_eq!(
            f_resize_samples_allow_finite_fma(&x86_fma, samples),
            expected_finite_samples,
            "finite sample admission differs for {filter:?}",
        );
        if expected_finite_samples {
            assert!(f_resize_coefficients_allow_finite_fma(&horizontal));
            assert!(f_resize_coefficients_allow_finite_fma(&vertical));
        }

        let actual = pil_resize_f_boxed_with_fma::<_, true>(
            &image, 17, 9, bounds.0, bounds.1, bounds.2, bounds.3, filter, &x86_fma,
        );
        let portable = pil_resize_f_boxed_with_fma::<_, true>(
            &image,
            17,
            9,
            bounds.0,
            bounds.1,
            bounds.2,
            bounds.3,
            filter,
            &PortableFma,
        );
        assert_eq!(
            actual.as_bytes(),
            portable.as_bytes(),
            "boxed F output differs for {filter:?}",
        );
    }
}
