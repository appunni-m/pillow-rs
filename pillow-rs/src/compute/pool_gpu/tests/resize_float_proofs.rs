use super::*;

#[test]
fn f_resize_constant_lowering_requires_exact_finite_source() {
    let resize = PipelineOp::Resize {
        w: 2,
        h: 3,
        filter: ResampleFilter::Bicubic,
    };
    let bits = 37.0f32.to_bits();
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, 37.0f32.to_le_bytes().repeat(6)).unwrap(),
    );
    assert_eq!(
        gpu_f_resize_constant_bits(std::slice::from_ref(&resize), &image, Some("F")),
        Some(bits)
    );

    let mut mixed = 37.0f32.to_le_bytes().repeat(6);
    mixed[4..8].copy_from_slice(&113.0f32.to_le_bytes());
    let mixed = DynamicImage::ImageRgba8(RgbaImage::from_raw(3, 2, mixed).unwrap());
    assert_eq!(
        gpu_f_resize_constant_bits(std::slice::from_ref(&resize), &mixed, Some("F")),
        None
    );

    let nearest = PipelineOp::Resize {
        w: 2,
        h: 3,
        filter: ResampleFilter::Nearest,
    };
    assert_eq!(
        gpu_f_resize_constant_bits(std::slice::from_ref(&nearest), &image, Some("F")),
        None
    );

    let negative_zero = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, (-0.0f32).to_le_bytes().repeat(6)).unwrap(),
    );
    assert_eq!(
        gpu_f_resize_constant_bits(std::slice::from_ref(&resize), &negative_zero, Some("F")),
        None
    );
}

#[test]
fn f_nearest_resize_uses_cumulative_coefficients() {
    assert!(gpu_resize_nearest_uses_coefficients(Some("F")));
    let coefficients = gpu_resize_coefficients(7, 2, ResampleFilter::Nearest);
    assert_eq!(&coefficients.xmin, &[0, 0, 0, 0, 1, 1, 1]);
    assert!(coefficients.count.iter().all(|&count| count == 1));
    assert!(
        coefficients
            .weights
            .iter()
            .all(|&weight| weight == 1i64 << 22)
    );
}

#[test]
fn f_resize_box_copy_lowering_requires_non_downscaling_box_chain() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            3,
            2,
            [1.0f32, -2.5, 3.75, 4.5, -7.0, 9.25]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    let box_resize = |w, h| PipelineOp::Resize {
        w,
        h,
        filter: ResampleFilter::Box,
    };

    assert!(gpu_f_resize_box_copy_is_exact(
        &[box_resize(6, 4)],
        &image,
        Some("F")
    ));
    assert!(gpu_f_resize_box_copy_is_exact(
        &[box_resize(3, 2), box_resize(12, 8)],
        &image,
        Some("F")
    ));
    assert!(gpu_f_resize_box_copy_is_exact(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::from(vec![0u8; 24]),
                mode: PixelMode::F,
            },
            box_resize(6, 4),
        ],
        &image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::from(vec![0u8; 4]),
                mode: PixelMode::F,
            },
            box_resize(6, 4),
        ],
        &image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[box_resize(2, 2)],
        &image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[PipelineOp::Resize {
            w: 6,
            h: 4,
            filter: ResampleFilter::Bilinear,
        }],
        &image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[box_resize(6, 4)],
        &image,
        Some("I")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[box_resize(6, 4), PipelineOp::Mirror],
        &image,
        Some("F")
    ));

    let negative_zero = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, (-0.0f32).to_le_bytes().repeat(6)).unwrap(),
    );
    assert!(gpu_f_resize_box_copy_is_exact(
        &[box_resize(6, 4)],
        &negative_zero,
        Some("F")
    ));
    let nan = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, f32::NAN.to_le_bytes().repeat(6)).unwrap(),
    );
    assert!(gpu_f_resize_box_copy_is_exact(
        &[box_resize(6, 4)],
        &nan,
        Some("F")
    ));
    let infinity = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, f32::INFINITY.to_le_bytes().repeat(6)).unwrap(),
    );
    assert!(gpu_f_resize_box_copy_is_exact(
        &[box_resize(6, 4)],
        &infinity,
        Some("F")
    ));
}

#[test]
fn f_resize_box_average_lowering_requires_2x_axes_and_safe_values() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            4,
            4,
            [
                1.0f32, 2.5, 3.75, 4.5, 7.0, 9.25, 12.0, 15.5, 2.0, 3.5, 5.75, 6.5, 8.0, 10.25,
                13.0, 16.5,
            ]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
        )
        .unwrap(),
    );
    let coefficients = super::gpu_resize_coefficients(2, 4, ResampleFilter::Box);
    assert_eq!(coefficients.count, [2, 2]);
    assert_eq!(coefficients.weights, [1 << 21, 1 << 21, 1 << 21, 1 << 21]);

    assert_box_average_geometry(&image);
    assert_box_average_value_domain();
}

fn box_average_resize(w: u32, h: u32) -> PipelineOp {
    PipelineOp::Resize {
        w,
        h,
        filter: ResampleFilter::Box,
    }
}

fn assert_box_average_geometry(image: &DynamicImage) {
    assert!(gpu_f_resize_box_average_is_exact(
        &[box_average_resize(2, 4)],
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_box_average_is_exact(
        &[box_average_resize(4, 2)],
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_box_average_is_exact(
        &[box_average_resize(2, 2)],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_average_is_exact(
        &[box_average_resize(3, 4)],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_average_is_exact(
        &[box_average_resize(2, 4), box_average_resize(1, 4)],
        image,
        Some("F")
    ));
}

fn assert_box_average_value_domain() {
    let mixed_sign = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            4,
            4,
            [
                1.0f32, -2.5, 3.75, 4.5, 7.0, 9.25, 12.0, 15.5, 2.0, 3.5, 5.75, 6.5, 8.0, 10.25,
                13.0, 16.5,
            ]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
        )
        .unwrap(),
    );
    assert!(!gpu_f_resize_box_average_is_exact(
        &[box_average_resize(2, 4)],
        &mixed_sign,
        Some("F")
    ));

    let subnormal = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(4, 4, (f32::MIN_POSITIVE).to_le_bytes().repeat(16)).unwrap(),
    );
    assert!(!gpu_f_resize_box_average_is_exact(
        &[box_average_resize(2, 4)],
        &subnormal,
        Some("F")
    ));
    let floor = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(4, 4, f32::from_bits(0x3580_0000).to_le_bytes().repeat(16)).unwrap(),
    );
    assert!(gpu_f_resize_box_average_is_exact(
        &[box_average_resize(2, 4)],
        &floor,
        Some("F")
    ));
    let below_floor = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            4,
            4,
            [f32::from_bits(0x3500_0000)]
                .into_iter()
                .chain([f32::from_bits(0x3580_0000); 15])
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    assert!(!gpu_f_resize_box_average_is_exact(
        &[box_average_resize(2, 4)],
        &below_floor,
        Some("F")
    ));
    let negative_zero = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(4, 4, (-0.0f32).to_le_bytes().repeat(16)).unwrap(),
    );
    assert!(!gpu_f_resize_box_average_is_exact(
        &[box_average_resize(2, 4)],
        &negative_zero,
        Some("F")
    ));
}

#[test]
fn f_resize_dyadic_lowering_requires_proven_filter_and_source_domain() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            4,
            2,
            [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    let bilinear = PipelineOp::Resize {
        w: 8,
        h: 4,
        filter: ResampleFilter::Bilinear,
    };
    assert_f_resize_dyadic_basic_cases(&image, &bilinear);
    assert_f_resize_dyadic_box_chain_cases(&image);
    assert_f_resize_dyadic_arithmetic_filter_boundaries();
    assert_f_resize_dyadic_geometry_rejections(&image);
    assert_f_resize_dyadic_source_word_rejections(&bilinear);
    assert!(!gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(&bilinear),
        &image,
        Some("I")
    ));
}

fn dyadic_box_resize(w: u32, h: u32) -> PipelineOp {
    PipelineOp::Resize {
        w,
        h,
        filter: ResampleFilter::Box,
    }
}

fn assert_f_resize_dyadic_basic_cases(image: &DynamicImage, bilinear: &PipelineOp) {
    // Bilinear 2x rows contain only exact dyadic weights.  A 4:1 Box
    // reduction has four equal dyadic taps; both exercise marker 6.
    assert!(gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(bilinear),
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_dyadic_is_exact(
        &[dyadic_box_resize(1, 2)],
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_dyadic_is_exact(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::copy_from_slice(image.as_bytes()),
                mode: PixelMode::F,
            },
            dyadic_box_resize(1, 2),
        ],
        image,
        Some("F")
    ));
}

fn assert_f_resize_dyadic_box_chain_cases(image: &DynamicImage) {
    // Two changed power-of-two Box axes remain exact because every
    // horizontal dyadic average is exactly representable before the
    // vertical pass consumes it.
    assert!(gpu_f_resize_dyadic_is_exact(
        &[dyadic_box_resize(1, 1)],
        image,
        Some("F")
    ));
    // Chained Box reductions retain exactness when the cumulative dyadic
    // shift stays within the same 24-bit significand bound.  The second
    // resize consumes f32 intermediates that are no longer single powers
    // of two, so this guards the chain proof rather than the one-pass
    // source-word check above.
    assert!(gpu_f_resize_dyadic_is_exact(
        &[dyadic_box_resize(2, 1), dyadic_box_resize(1, 1)],
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_dyadic_is_exact(
        &[
            dyadic_box_resize(3, 2),
            PipelineOp::PutData {
                data: bytes::Bytes::from(
                    [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0]
                        .into_iter()
                        .flat_map(f32::to_le_bytes)
                        .collect::<Vec<_>>(),
                ),
                mode: PixelMode::F,
            },
            dyadic_box_resize(2, 1),
            dyadic_box_resize(1, 1),
        ],
        image,
        Some("F")
    ));
    // Mixing an arithmetic filter into the chain is still outside the
    // proof: only Box's power-of-two scaling preserves the exact value
    // domain after an intermediate f32 store.
    assert!(!gpu_f_resize_dyadic_is_exact(
        &[
            dyadic_box_resize(2, 1),
            PipelineOp::Resize {
                w: 1,
                h: 1,
                filter: ResampleFilter::Bilinear,
            },
        ],
        image,
        Some("F")
    ));
    // Non-divisor Box geometry can still be dyadic when each output row
    // uses only one or two equal taps.  The 4 -> 3 table is [1, 2, 1]
    // taps, so it exercises the generalized row proof rather than the
    // integral power-of-two ratio shortcut.
    assert!(gpu_f_resize_dyadic_is_exact(
        &[dyadic_box_resize(3, 4)],
        image,
        Some("F")
    ));
}

fn assert_f_resize_dyadic_arithmetic_filter_boundaries() {
    // The smallest two-sample reduction has exact, non-negative 1/2
    // rows for these arithmetic filters.  Their kernels differ away
    // from this geometry, so this matrix guards the proof's row-level
    // filter admission rather than treating every arithmetic filter as
    // dyadic.
    let two_sample = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            2,
            2,
            [1.0f32, 2.0, 4.0, 8.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    for filter in [
        ResampleFilter::Bicubic,
        ResampleFilter::Lanczos,
        ResampleFilter::Hamming,
    ] {
        assert!(gpu_f_resize_dyadic_is_exact(
            &[PipelineOp::Resize { w: 1, h: 1, filter }],
            &two_sample,
            Some("F")
        ));
    }
    let three_sample = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            3,
            2,
            [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    // A 3 -> 2 arithmetic resize crosses the admission boundary: the
    // wider kernels need more than two taps, and Hamming's two taps are
    // non-dyadic.  Keep all three filters on host control here.
    for filter in [
        ResampleFilter::Bicubic,
        ResampleFilter::Lanczos,
        ResampleFilter::Hamming,
    ] {
        assert!(!gpu_f_resize_dyadic_is_exact(
            &[PipelineOp::Resize { w: 2, h: 1, filter }],
            &three_sample,
            Some("F")
        ));
    }
}

fn assert_f_resize_dyadic_geometry_rejections(image: &DynamicImage) {
    let non_dyadic_box_source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            7,
            1,
            [1.0f32, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    // The middle 7 -> 3 Box row has three 1/3 taps, which is outside the
    // dyadic device proof even though the fixed shader table is valid.
    assert!(!gpu_f_resize_dyadic_is_exact(
        &[dyadic_box_resize(3, 1)],
        &non_dyadic_box_source,
        Some("F")
    ));

    let wide_two_axis = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            4,
            2,
            [
                2.0f32.powi(-80),
                2.0f32.powi(-59),
                2.0f32.powi(-59),
                2.0f32.powi(-59),
                2.0f32.powi(-59),
                2.0f32.powi(-59),
                2.0f32.powi(-59),
                2.0f32.powi(-59),
            ]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect(),
        )
        .unwrap(),
    );
    // A second reduction would expose a 24-bit-plus significand span;
    // retain exact host control rather than relying on device rounding.
    assert!(!gpu_f_resize_dyadic_is_exact(
        &[dyadic_box_resize(1, 1)],
        &wide_two_axis,
        Some("F")
    ));
    assert!(!gpu_f_resize_dyadic_is_exact(
        &[PipelineOp::Resize {
            w: 8,
            h: 4,
            filter: ResampleFilter::Bicubic,
        }],
        image,
        Some("F")
    ));
}

fn assert_f_resize_dyadic_source_word_rejections(bilinear: &PipelineOp) {
    let rejects = [
        (
            [(-0.0f32), 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
            "negative-zero",
        ),
        ([f32::NAN, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0], "nan"),
        (
            [f32::INFINITY, 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
            "infinity",
        ),
        (
            [f32::from_bits(1), 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
            "subnormal",
        ),
        (
            [2.0f32.powi(-81), 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
            "below-coefficient-bound",
        ),
        (
            [2.0f32.powi(-16), 1.0, 2.0, 4.0, 8.0, 16.0, 32.0, 64.0],
            "wide-exponent-span",
        ),
    ];
    for (values, _label) in rejects {
        let rejected = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                4,
                2,
                values.into_iter().flat_map(f32::to_le_bytes).collect(),
            )
            .unwrap(),
        );
        assert!(
            !gpu_f_resize_dyadic_is_exact(std::slice::from_ref(bilinear), &rejected, Some("F")),
            "unexpected admission for {_label}"
        );
    }
}

#[test]
fn f_resize_f64_lowering_proves_finite_subnormal_words_and_two_axes() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            2,
            2,
            [0.1f32, -0.3, 1.7, 2.9]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    let horizontal = PipelineOp::Resize {
        w: 1,
        h: 2,
        filter: ResampleFilter::Bilinear,
    };
    assert_f_resize_f64_two_axis_proofs(&image, &horizontal);
    assert_f_resize_f64_putdata_proofs(&image);
    assert_f_resize_f64_mixed_geometry_proofs(&image);
    assert_f_resize_f64_special_value_proofs(&horizontal);
    assert_f_resize_f64_subnormal_word_proofs(&horizontal);
    assert!(!gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&horizontal),
        &image,
        Some("I")
    ));
}

fn assert_f_resize_f64_two_axis_proofs(image: &DynamicImage, horizontal: &PipelineOp) {
    assert!(gpu_f_resize_f64_is_exact(
        std::slice::from_ref(horizontal),
        image,
        Some("F")
    ));

    // The encoder separates the two reducers, and the host proof checks
    // the rounded horizontal words before proving the vertical pass.
    assert!(gpu_f_resize_f64_is_exact(
        &[PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Bilinear,
        }],
        image,
        Some("F")
    ));

    // A non-dyadic vertical table keeps this case outside marker 6 while
    // the exact f64 reducer still proves its two-axis intermediate.
    let non_dyadic_two_axis = PipelineOp::Resize {
        w: 1,
        h: 5,
        filter: ResampleFilter::Bilinear,
    };
    assert!(!gpu_f_resize_integer_is_exact(
        std::slice::from_ref(&non_dyadic_two_axis),
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&non_dyadic_two_axis),
        image,
        Some("F")
    ));
    let filtered_chain = [
        PipelineOp::Resize {
            w: 3,
            h: 3,
            filter: ResampleFilter::Bicubic,
        },
        PipelineOp::Resize {
            w: 2,
            h: 2,
            filter: ResampleFilter::Lanczos,
        },
    ];
    assert!(gpu_f_resize_f64_is_exact(&filtered_chain, image, Some("F")));
}

fn assert_f_resize_f64_putdata_proofs(image: &DynamicImage) {
    // PutData(F) replaces the deferred source words before the resize;
    // the marker-9 proof must validate and consume that replacement rather
    // than conservatively routing an otherwise exact non-dyadic resize.
    let putdata = bytes::Bytes::from(
        [0.1f32, -0.3, 1.7, 2.9]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>(),
    );
    assert!(gpu_f_resize_f64_is_exact(
        &[
            PipelineOp::PutData {
                data: putdata.clone(),
                mode: PixelMode::F,
            },
            PipelineOp::Resize {
                w: 1,
                h: 5,
                filter: ResampleFilter::Bilinear,
            },
        ],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_f64_is_exact(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::from(vec![0u8; 4]),
                mode: PixelMode::F,
            },
            PipelineOp::Resize {
                w: 1,
                h: 5,
                filter: ResampleFilter::Bilinear,
            },
        ],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_f64_is_exact(
        &[
            PipelineOp::PutData {
                data: putdata,
                mode: PixelMode::I,
            },
            PipelineOp::Resize {
                w: 1,
                h: 5,
                filter: ResampleFilter::Bilinear,
            },
        ],
        image,
        Some("F")
    ));
}

fn assert_f_resize_f64_mixed_geometry_proofs(image: &DynamicImage) {
    // The explicit horizontal/vertical compute-pass boundary also makes
    // the opposite mixed geometry safe: horizontal upscaling is fully
    // materialized before the vertical downscale consumes it.
    assert!(gpu_f_resize_f64_is_exact(
        &[PipelineOp::Resize {
            w: 4,
            h: 1,
            filter: ResampleFilter::Bilinear,
        }],
        image,
        Some("F")
    ));
    // Marker 6 and the broader dyadic Box proof retain their own
    // conservative mixed-geometry guard; marker 9's explicit pass
    // boundary is not a proof for those older reducers.
    let mixed_box_source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            1,
            2,
            [-9.7966165f32, 6.5041304]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    let mixed_box = PipelineOp::Resize {
        w: 2,
        h: 1,
        filter: ResampleFilter::Box,
    };
    assert!(!gpu_f_resize_integer_is_exact(
        std::slice::from_ref(&mixed_box),
        &mixed_box_source,
        Some("F")
    ));
    assert!(!gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(&mixed_box),
        &mixed_box_source,
        Some("F")
    ));
    assert!(gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&mixed_box),
        &mixed_box_source,
        Some("F")
    ));
}

fn assert_f_resize_f64_special_value_proofs(horizontal: &PipelineOp) {
    for value in [f32::NAN.to_bits(), f32::INFINITY.to_bits()] {
        let special = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                2,
                2,
                [value, 0x3f80_0000, 0x4000_0000, 0x4040_0000]
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect(),
            )
            .unwrap(),
        );
        assert!(
            gpu_f_resize_f64_is_exact(std::slice::from_ref(horizontal), &special, Some("F")),
            "finite-coefficient special-value rows should use the exact marker"
        );
    }
}

fn assert_f_resize_f64_subnormal_word_proofs(horizontal: &PipelineOp) {
    // Finite subnormal words use the explicit 2^-149 source scale and
    // remain eligible for the same exact integer reducer.  Keep both
    // signs here; the filtered path canonicalizes only an exact zero
    // result, while nonzero negative subnormals retain their sign bit.
    for values in [
        [
            f32::from_bits(1),
            f32::from_bits(2),
            f32::from_bits(3),
            f32::from_bits(4),
        ],
        [
            -f32::from_bits(1),
            -f32::from_bits(2),
            -f32::from_bits(3),
            -f32::from_bits(4),
        ],
    ] {
        let subnormal = DynamicImage::ImageRgba8(
            RgbaImage::from_raw(
                2,
                2,
                values.into_iter().flat_map(f32::to_le_bytes).collect(),
            )
            .unwrap(),
        );
        assert!(gpu_f_resize_f64_is_exact(
            std::slice::from_ref(horizontal),
            &subnormal,
            Some("F")
        ));
    }
}

#[test]
fn f_order_filter_putdata_requires_finite_words() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            2,
            2,
            [1.0f32, 2.0, 3.0, 4.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    let rank = PipelineOp::RankFilter { size: 3, rank: 4 };
    let words = |values: [f32; 4]| -> bytes::Bytes {
        bytes::Bytes::from(
            values
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect::<Vec<_>>(),
        )
    };

    assert!(gpu_float_filter_is_supported(
        &[
            PipelineOp::PutData {
                data: words([4.0, 3.0, 2.0, 1.0]),
                mode: PixelMode::F,
            },
            rank.clone(),
        ],
        &image,
    ));
    for special in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(!gpu_float_filter_is_supported(
            &[
                PipelineOp::PutData {
                    data: words([special, 3.0, 2.0, 1.0]),
                    mode: PixelMode::F,
                },
                rank.clone(),
            ],
            &image,
        ));
    }
    assert!(!gpu_float_filter_is_supported(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::from(vec![0u8; 4]),
                mode: PixelMode::F,
            },
            rank.clone(),
        ],
        &image,
    ));
    assert!(!gpu_float_filter_is_supported(
        &[
            PipelineOp::PutData {
                data: words([4.0, 3.0, 2.0, 1.0]),
                mode: PixelMode::I,
            },
            rank,
        ],
        &image,
    ));
}

#[test]
fn f_resize_f64_subnormal_rounding_is_ties_to_even() {
    assert_eq!(gpu_f64_integer_to_f32(1, -149), Some(0x0000_0001));
    assert_eq!(gpu_f64_integer_to_f32(1, -150), Some(0));
    assert_eq!(gpu_f64_integer_to_f32(3, -151), Some(0x0000_0001));
    assert_eq!(gpu_f64_integer_to_f32(3, -150), Some(0x0000_0002));
    assert_eq!(
        gpu_f64_integer_to_f32((1i128 << 23) - 1, -149),
        Some(0x007f_ffff)
    );
    assert_eq!(gpu_f64_integer_to_f32(1i128 << 23, -149), Some(0x0080_0000));
    assert_eq!(gpu_f64_integer_to_f32(-1, -149), Some(0x8000_0001));
    assert_eq!(gpu_f64_integer_to_f32(-1, -200), Some(0x8000_0000));
    assert_eq!(gpu_f64_integer_to_f32(1, 128), Some(0x7f80_0000));
    assert_eq!(gpu_f64_integer_to_f32(-1, 128), Some(0xff80_0000));
}

#[test]
fn f_resize_integer_lowering_proves_signed_arbitrary_significands_and_two_axes() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            2,
            2,
            [0.1f32, 0.3, 1.7, 2.9]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    let one_axis = PipelineOp::Resize {
        w: 1,
        h: 2,
        filter: ResampleFilter::Bilinear,
    };
    assert!(gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(&one_axis),
        &image,
        Some("F")
    ));

    // The horizontal integer proof materializes the exact rounded f32
    // intermediate before proving the vertical pass.
    assert!(gpu_f_resize_dyadic_is_exact(
        &[PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Bilinear,
        }],
        &image,
        Some("F")
    ));

    let mixed_sign = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            2,
            2,
            [0.1f32, -0.3, 1.7, 2.9]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    assert!(gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(&one_axis),
        &mixed_sign,
        Some("F")
    ));
}
