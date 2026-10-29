use super::execute_reduce;
use crate::raster::{DynamicImage, RgbImage, RgbaImage};

#[test]
fn rgb_reduce_specialized_rows_match_generic_reference_at_edges() {
    for (width, height, x_factor, y_factor) in [
        (5, 4, 3, 2),
        (7, 11, 3, 5),
        (4, 7, 1, 3),
        (13, 8, 4, 1),
        (2, 2, 2, 2),
        (4, 2, 2, 2),
        (8, 6, 2, 2),
        (32, 24, 2, 2),
        (9, 5, 2, 2),
    ] {
        let source = (0..width as usize * height as usize * 3)
            .map(|index| ((index * 37 + index / 11 * 17 + 29) % 256) as u8)
            .collect();
        let image = DynamicImage::ImageRgb8(
            RgbImage::from_raw(width, height, source).expect("RGB source shape must be valid"),
        );
        let optimized = execute_reduce(&image, x_factor, y_factor, Some("RGB"))
            .expect("specialized RGB Reduce must succeed");
        let reference = execute_reduce(&image, x_factor, y_factor, None)
            .expect("generic RGB Reduce reference must succeed");

        assert_eq!(
            optimized.as_bytes(),
            reference.as_bytes(),
            "RGB Reduce mismatch for {width}×{height} by {x_factor}×{y_factor}"
        );
    }
}
#[test]
fn rgba_reduce_native_channel_sums_match_generic_reference_at_edges() {
    for (width, height, x_factor, y_factor) in [
        (32, 24, 4, 3),
        (34, 27, 4, 3),
        (19, 23, 3, 5),
        (11, 9, 2, 2),
    ] {
        let source = (0..width as usize * height as usize * 4)
            .map(|index| ((index * 73 + index / 13 * 41 + 19) % 256) as u8)
            .collect();
        let image = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(width, height, source).expect("RGBA source shape must be valid"),
        );
        let optimized = execute_reduce(&image, x_factor, y_factor, Some("RGBA"))
            .expect("native RGBA Reduce must succeed");
        let reference = execute_reduce(&image, x_factor, y_factor, None)
            .expect("generic RGBA Reduce reference must succeed");

        assert_eq!(
            optimized.as_bytes(),
            reference.as_bytes(),
            "RGBA Reduce mismatch for {width}×{height} by {x_factor}×{y_factor}"
        );
    }
}

#[test]
fn rgb_sparse_2x2_reduction_matches_generic_and_rejects_dense_input() {
    let (width, height) = (1024, 768);
    let zero = DynamicImage::ImageRgb8(RgbImage::new(width, height));
    let zero_sparse = super::execute_reduce_rgb_sparse_2x2(&zero, width / 2, height / 2)
        .expect("all-zero RGB reduction must succeed")
        .expect("all-zero RGB should use the sparse path");
    let zero_reference =
        execute_reduce(&zero, 2, 2, None).expect("generic zero RGB reduction must succeed");
    assert_eq!(zero_sparse.as_bytes(), zero_reference.as_bytes());

    let mut source = vec![0; width as usize * height as usize * 3];
    for (x, y, color) in [
        (0, 0, [1, 2, 3]),
        (1, 1, [255, 17, 9]),
        (700, 300, [0, 129, 0]),
        (1023, 767, [12, 34, 56]),
    ] {
        let index = (y * width + x) as usize * 3;
        source[index..index + 3].copy_from_slice(&color);
    }
    let image = DynamicImage::ImageRgb8(
        RgbImage::from_raw(width, height, source).expect("RGB source shape must be valid"),
    );

    let sparse = super::execute_reduce_rgb_sparse_2x2(&image, width / 2, height / 2)
        .expect("sparse RGB reduction must succeed")
        .expect("few nonzero RGB pixels should use the sparse path");
    let routed = execute_reduce(&image, 2, 2, Some("RGB")).expect("RGB Reduce must succeed");
    let reference =
        execute_reduce(&image, 2, 2, None).expect("generic RGB Reduce reference must succeed");
    assert_eq!(sparse.as_bytes(), reference.as_bytes());
    assert_eq!(routed.as_bytes(), reference.as_bytes());

    let (tail_width, tail_height) = (1022, 514);
    let mut tail_source = vec![0; tail_width as usize * tail_height as usize * 3];
    let last_pixel = (tail_width as usize * tail_height as usize - 1) * 3;
    tail_source[last_pixel..last_pixel + 3].copy_from_slice(&[19, 37, 251]);
    let tail_image = DynamicImage::ImageRgb8(
        RgbImage::from_raw(tail_width, tail_height, tail_source)
            .expect("tail RGB source shape must be valid"),
    );
    let tail_sparse = super::execute_reduce_rgb_sparse_2x2(
        &tail_image,
        tail_width.div_ceil(2),
        tail_height.div_ceil(2),
    )
    .expect("sparse RGB scan tail must succeed")
    .expect("sparse RGB scan with a partial eight-pixel tail should be selected");
    let tail_reference = execute_reduce(&tail_image, 2, 2, None)
        .expect("generic odd-size RGB reduction must succeed");
    assert_eq!(tail_sparse.as_bytes(), tail_reference.as_bytes());

    let dense = DynamicImage::ImageRgb8(
        RgbImage::from_raw(width, height, vec![1; width as usize * height as usize * 3])
            .expect("dense RGB source shape must be valid"),
    );
    assert!(
        super::execute_reduce_rgb_sparse_2x2(&dense, width / 2, height / 2)
            .expect("density probe must succeed")
            .is_none(),
        "dense RGB inputs must retain the established reducer"
    );
}
