use super::{ScanEvidence, VECTOR_BYTES, prove_identity};

fn gaussian_parameters(radius: f32) -> (usize, u32, u32) {
    crate::compute::helpers::filters::box_blur_parameters(radius)
}

fn rgba(width: usize, height: usize, pixel: [u8; 4]) -> Vec<u8> {
    pixel.repeat(width * height)
}

#[test]
fn vector_scan_proves_large_uniform_identity_and_counts_tail() {
    let parameters = gaussian_parameters(
        crate::compute::helpers::filters::gaussian_blur_three_pass_radius(7.5),
    );
    let raw = rgba(257, 257, [83, 149, 17, 191]);
    let evidence = prove_identity(&raw, 257, 257, parameters, parameters);

    assert!(evidence.identity);
    assert_eq!(evidence.vector_blocks, (raw.len() / 16) as u64);
    assert_eq!(evidence.scalar_tail_pixels, 1);
}

#[test]
fn vector_scan_rejects_an_unsampled_pixel_outside_the_probe_grid() {
    let parameters = gaussian_parameters(
        crate::compute::helpers::filters::gaussian_blur_three_pass_radius(7.5),
    );
    let mut raw = rgba(257, 256, [83, 149, 17, 191]);
    raw[(257 + 1) * 4] ^= 1;
    let evidence = prove_identity(&raw, 257, 256, parameters, parameters);

    assert!(!evidence.identity);
    assert!(evidence.vector_blocks > 0);
    assert_eq!(evidence.scalar_tail_pixels, 0);
}

#[test]
fn non_identity_constant_skips_the_full_scan() {
    let raw = rgba(257, 257, [255, 255, 255, 255]);
    let evidence = prove_identity(&raw, 257, 257, (7, 1, 1), (7, 1, 1));

    assert_eq!(evidence, ScanEvidence::default());
}

#[test]
fn minimum_area_rejects_just_below_and_accepts_at_the_threshold() {
    let parameters = gaussian_parameters(
        crate::compute::helpers::filters::gaussian_blur_three_pass_radius(7.5),
    );
    let pixel = [83, 149, 17, 191];

    let below = rgba(255, 256, pixel);
    assert_eq!(
        prove_identity(&below, 255, 256, parameters, parameters),
        ScanEvidence::default()
    );

    let at_threshold = rgba(256, 256, pixel);
    let evidence = prove_identity(&at_threshold, 256, 256, parameters, parameters);
    assert!(evidence.identity);
    assert_eq!(evidence.vector_blocks, (at_threshold.len() / VECTOR_BYTES) as u64);
}
