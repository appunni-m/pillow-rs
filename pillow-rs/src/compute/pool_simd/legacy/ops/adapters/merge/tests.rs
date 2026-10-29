//! Focused contract tests for SIMD Image.merge.

#[test]
fn native_rgb_merge_borrows_auxiliary_bands_from_full_channel_list() {
    use crate::pipeline::ColorMode;
    use crate::raster::{DynamicImage, GrayImage};

    let red = vec![1, 2, 3, 4, 5];
    let green = vec![11, 22, 33, 44, 55];
    let blue = vec![101, 102, 103, 104, 105];
    let first = DynamicImage::ImageLuma8(
        GrayImage::from_raw(5, 1, red.clone()).expect("first L band shape"),
    );
    let bands = vec![
        crate::image::Image::from_luma_mask(5, 1, red).expect("first L band"),
        crate::image::Image::from_luma_mask(5, 1, green.clone()).expect("green L band"),
        crate::image::Image::from_luma_mask(5, 1, blue).expect("blue L band"),
    ];
    let shared_green = bands[1]
        .materialized_shared_for_ops()
        .expect("green L band materialization");
    let owners = super::native_merge_rgb_luma_owners(&first, &ColorMode::RGB, &bands, "RGB", 5)
        .expect("RGB merge owner lookup")
        .expect("all three channel bands should select the borrowed path");

    assert_eq!(owners.len(), 2, "the first band is already `img`");
    assert!(std::sync::Arc::ptr_eq(&owners[0], &shared_green));

    let (merged, vector_blocks, scalar_tail, path) =
        super::native_merge_bytes(&first, &ColorMode::RGB, &bands, Some("L"), "RGB")
            .expect("native SIMD merge")
            .expect("native RGB/L merge should be admitted");
    assert_eq!(
        merged.as_bytes(),
        [1, 11, 101, 2, 22, 102, 3, 33, 103, 4, 44, 104, 5, 55, 105]
    );
    assert_eq!(vector_blocks, 1);
    assert_eq!(scalar_tail, 3);
    assert_eq!(path, "wide-shuffle");
}

#[cfg(target_arch = "x86_64")]
#[test]
fn ssse3_native_rgb_merge_interleaves_exact_blocks_and_tails() {
    if !std::is_x86_feature_detected!("ssse3") {
        return;
    }

    for pixels in [0usize, 1, 3, 4, 15, 16, 17, 31, 32, 33, 1024] {
        let red = (0..pixels)
            .map(|index| ((index * 7 + 1) & 255) as u8)
            .collect::<Vec<_>>();
        let green = (0..pixels)
            .map(|index| ((index * 11 + 3) & 255) as u8)
            .collect::<Vec<_>>();
        let blue = (0..pixels)
            .map(|index| ((index * 13 + 5) & 255) as u8)
            .collect::<Vec<_>>();
        let expected = (0..pixels)
            .flat_map(|index| [red[index], green[index], blue[index]])
            .collect::<Vec<_>>();
        let bands = [&red[..], &green[..], &blue[..]];

        let (actual, vector_blocks, scalar_tail, path) =
            super::native_merge_interleave(pixels as u32, 1, pixels, 3, &bands)
                .expect("native RGB merge should succeed");

        assert_eq!(actual.as_bytes(), expected, "pixel count {pixels}");
        if pixels >= 16 {
            assert_eq!(vector_blocks, (pixels / 16) as u64);
            assert_eq!(scalar_tail, ((pixels % 16) * 3) as u64);
            assert_eq!(path, "ssse3-interleave");
        } else {
            assert_eq!(vector_blocks, (pixels / 4) as u64);
            assert_eq!(scalar_tail, ((pixels % 4) * 3) as u64);
            assert_eq!(path, "wide-shuffle");
        }
    }
}
