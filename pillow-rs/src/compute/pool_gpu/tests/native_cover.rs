use super::*;

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_cover_input_requires_matching_byte_layout() {
    let cover = PipelineOp::Cover {
        w: 8,
        h: 12,
        filter: ResampleFilter::Bicubic,
    };
    let cases = [
        (
            DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![3, 5]).unwrap()),
            Some("L"),
            Some(1),
        ),
        (
            DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(2, 1, vec![3, 19, 5, 23]).unwrap()),
            Some("LA"),
            Some(2),
        ),
        (
            DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17]).unwrap()),
            Some("RGB"),
            Some(3),
        ),
        (
            DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17]).unwrap()),
            Some("HSV"),
            Some(3),
        ),
    ];
    for (image, mode, expected) in cases {
        assert_eq!(
            super::gpu_native_cover_input_channels(std::slice::from_ref(&cover), &image, mode,),
            expected,
            "mode {mode:?}"
        );
    }

    let rgba = DynamicImage::ImageRgba8(RgbaImage::from_raw(1, 1, vec![3, 5, 7, 11]).unwrap());
    assert_eq!(
        super::gpu_native_cover_input_channels(std::slice::from_ref(&cover), &rgba, Some("CMYK"),),
        None,
        "four-byte layouts already match the generic transport"
    );
    let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![1, 2, 3]).unwrap());
    assert_eq!(
        super::gpu_native_cover_input_channels(std::slice::from_ref(&cover), &rgb, Some("CMYK"),),
        None,
        "logical mode must match physical storage"
    );
    let multiple = [cover, PipelineOp::Duplicate];
    assert_eq!(
        super::gpu_native_cover_input_channels(&multiple, &rgb, Some("RGB")),
        None,
        "native source packing is singleton-only"
    );
}

#[test]
fn native_cover_packed_output_dispatch_covers_row_tails_and_checks_limits() {
    for channels in [1u8, 2] {
        for width in [1u32, 2, 3, 4, 5, 63, 64, 65] {
            for height in [1u32, 15, 16, 17] {
                let source_height = height;
                let plan = super::plan_native_cover_packed_output_dispatch(
                    width,
                    height,
                    source_height,
                    channels,
                    65_535,
                    u32::MAX,
                    u64::MAX,
                    u64::MAX,
                )
                .expect("bounded native L/LA Cover output should be planned");
                let pixels_per_word = 4 / u32::from(channels);
                let words_per_row = width.div_ceil(pixels_per_word);
                let expected_words = words_per_row * height;
                assert_eq!(plan.channels, channels);
                assert_eq!(plan.words_per_row, words_per_row);
                assert_eq!(plan.word_count, expected_words);
                assert_eq!(plan.transfer_bytes, u64::from(expected_words) * 4);
                assert_eq!(plan.groups_x, words_per_row.div_ceil(16));
                assert_eq!(plan.groups_y, height.div_ceil(16));
                assert_eq!(plan.horizontal_words_per_row, words_per_row);
                assert_eq!(plan.horizontal_word_count, words_per_row * source_height);
                assert_eq!(
                    plan.horizontal_transfer_bytes,
                    u64::from(plan.horizontal_word_count) * 4
                );
                assert_eq!(plan.horizontal_groups_x, words_per_row.div_ceil(16));
                assert_eq!(plan.horizontal_groups_y, source_height.div_ceil(16));

                let mut word_seen = vec![false; expected_words as usize];
                let mut pixel_writes = vec![0u8; (width * height) as usize];
                for y in 0..plan.groups_y * 16 {
                    for x in 0..plan.groups_x * 16 {
                        if x >= words_per_row || y >= height {
                            continue;
                        }
                        let word_index = y * words_per_row + x;
                        assert!(!word_seen[word_index as usize]);
                        word_seen[word_index as usize] = true;
                        for lane in 0..pixels_per_word {
                            let output_x = x * pixels_per_word + lane;
                            if output_x < width {
                                let pixel_index = y * width + output_x;
                                pixel_writes[pixel_index as usize] += 1;
                            }
                        }
                    }
                }
                assert!(word_seen.into_iter().all(|seen| seen));
                assert!(pixel_writes.into_iter().all(|writes| writes == 1));
            }
        }
    }

    for (channels, one_group_width) in [(1, 64), (2, 32)] {
        let exact = super::plan_native_cover_packed_output_dispatch(
            one_group_width,
            16,
            16,
            channels,
            1,
            u32::MAX,
            u64::MAX,
            u64::MAX,
        )
        .expect("one workgroup per axis should fit the boundary");
        assert_eq!((exact.groups_x, exact.groups_y), (1, 1));
        assert!(
            super::plan_native_cover_packed_output_dispatch(
                one_group_width + 1,
                16,
                16,
                channels,
                1,
                u32::MAX,
                u64::MAX,
                u64::MAX,
            )
            .is_none()
        );
    }
    assert!(
        super::plan_native_cover_packed_output_dispatch(
            64,
            17,
            16,
            1,
            1,
            u32::MAX,
            u64::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(
        super::plan_native_cover_packed_output_dispatch(1, 1, 1, 1, 1, 3, u64::MAX, u64::MAX,)
            .is_none()
    );
    assert!(
        super::plan_native_cover_packed_output_dispatch(1, 1, 1, 1, 1, u32::MAX, 3, u64::MAX,)
            .is_none()
    );
    assert!(
        super::plan_native_cover_packed_output_dispatch(1, 1, 1, 1, 1, u32::MAX, u64::MAX, 3,)
            .is_none()
    );
    assert!(
        super::plan_native_cover_packed_output_dispatch(
            u32::MAX,
            2,
            2,
            1,
            65_535,
            u32::MAX,
            u64::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(
        super::plan_native_cover_packed_output_dispatch(
            1,
            1,
            1,
            3,
            65_535,
            u32::MAX,
            u64::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(
        super::plan_native_cover_packed_output_dispatch(
            1,
            1,
            1,
            1,
            0,
            u32::MAX,
            u64::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(
        super::plan_native_cover_packed_output_dispatch(
            1,
            1,
            0,
            1,
            1,
            u32::MAX,
            u64::MAX,
            u64::MAX,
        )
        .is_none()
    );
}

#[test]
fn native_cover_packed_output_admits_singleton_l_and_la_resize() {
    let bicubic = PipelineOp::Resize {
        w: 21,
        h: 13,
        filter: ResampleFilter::Bicubic,
    };
    let nearest = PipelineOp::Resize {
        w: 10,
        h: 7,
        filter: ResampleFilter::Nearest,
    };
    let limits = (65_535, u32::MAX, u64::MAX, 1024);
    assert!(
        super::gpu_native_cover_packed_output_dispatch(
            std::slice::from_ref(&bicubic),
            Some(1),
            Some("L"),
            8,
            limits.0,
            limits.1,
            limits.2,
            limits.3,
        )
        .is_some()
    );
    assert!(
        super::gpu_native_cover_packed_output_dispatch(
            std::slice::from_ref(&nearest),
            Some(1),
            Some("L"),
            8,
            limits.0,
            limits.1,
            limits.2,
            limits.3,
        )
        .is_some()
    );
    assert!(
        super::gpu_native_cover_packed_output_dispatch(
            std::slice::from_ref(&bicubic),
            Some(2),
            Some("LA"),
            8,
            limits.0,
            limits.1,
            limits.2,
            limits.3,
        )
        .is_some()
    );
    assert!(
        super::gpu_native_cover_packed_output_dispatch(
            std::slice::from_ref(&bicubic),
            Some(1),
            Some("RGB"),
            8,
            limits.0,
            limits.1,
            limits.2,
            limits.3,
        )
        .is_none()
    );
    assert!(
        super::gpu_native_cover_packed_output_dispatch(
            &[bicubic, PipelineOp::Duplicate],
            Some(1),
            Some("L"),
            8,
            limits.0,
            limits.1,
            limits.2,
            limits.3,
        )
        .is_none()
    );
}

#[test]
fn native_cover_packed_readback_removes_each_rows_padding() {
    let dispatch = super::plan_native_cover_packed_output_dispatch(
        5,
        3,
        3,
        1,
        65_535,
        u32::MAX,
        u64::MAX,
        u64::MAX,
    )
    .unwrap();
    let packed = [
        1, 2, 3, 4, 5, 0, 0, 0, 6, 7, 8, 9, 10, 0, 0, 0, 11, 12, 13, 14, 15, 0, 0, 0,
    ];
    assert_eq!(
        super::unpack_native_cover_packed_rows(5, 3, 1, dispatch, &packed).unwrap(),
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
    );
    assert!(super::unpack_native_cover_packed_rows(5, 3, 1, dispatch, &packed[..20]).is_err());

    let la_dispatch = super::plan_native_cover_packed_output_dispatch(
        5,
        3,
        3,
        2,
        65_535,
        u32::MAX,
        u64::MAX,
        u64::MAX,
    )
    .unwrap();
    let la_packed = [
        1, 101, 2, 102, 3, 103, 4, 104, 5, 105, 0, 0, 6, 106, 7, 107, 8, 108, 9, 109, 10, 110, 0,
        0, 11, 111, 12, 112, 13, 113, 14, 114, 15, 115, 0, 0,
    ];
    assert_eq!(
        super::unpack_native_cover_packed_rows(5, 3, 2, la_dispatch, &la_packed).unwrap(),
        [
            1, 101, 2, 102, 3, 103, 4, 104, 5, 105, 6, 106, 7, 107, 8, 108, 9, 109, 10, 110, 11,
            111, 12, 112, 13, 113, 14, 114, 15, 115,
        ]
    );
    assert!(super::unpack_native_cover_packed_rows(5, 3, 1, la_dispatch, &la_packed).is_err());
}
