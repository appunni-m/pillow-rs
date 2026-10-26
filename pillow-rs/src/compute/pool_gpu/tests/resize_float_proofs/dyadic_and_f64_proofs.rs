use super::{
    gpu_f_resize_dyadic_is_exact, gpu_f_resize_f64_is_exact, gpu_f_resize_integer_is_exact,
};
use crate::pipeline::{PipelineOp, PixelMode, ResampleFilter};
use crate::raster::{DynamicImage, RgbaImage};

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
