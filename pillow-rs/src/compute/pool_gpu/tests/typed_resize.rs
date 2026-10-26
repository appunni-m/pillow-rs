use super::*;

#[test]
fn typed_luma16_nearest_affine_transform_uses_native_word_path() {
    // I;16 affine transforms use Pillow's native nearest sampler even
    // when a non-nearest filter token is supplied.  The mode-5 shader
    // must therefore relocate the complete low 16-bit word, while its
    // affine plan uses the typed `floor(source + 0.5)` coordinate
    // contract rather than the byte path's destination-center offset.
    let values = [
        0x0102u16, 0x1122, 0x2233, 0x3344, 0x4455, 0x5566, 0x6677, 0x7788, 0x8899, 0x99aa, 0xaabb,
        0xbbcc,
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
        let bytes = values
            .iter()
            .flat_map(|value| {
                if mode == "I;16B" {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                }
            })
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, (4, 3), &bytes)
            .unwrap_or_else(|error| panic!("{mode} source: {error}"));
        let data = TransformData::Affine(vec![0.5, 0.0, 0.0, 0.0, 0.5, 0.0]);
        let transformed = source
            .transform_public(
                (3, 2),
                0,
                Some(data),
                3,
                0,
                Some(TransformFill::Scalar(258)),
            )
            .unwrap_or_else(|error| panic!("{mode} transform: {error}"));
        let expected = transformed
            .clone()
            .use_backend(Backend::Cpu)
            .tobytes()
            .unwrap_or_else(|error| panic!("{mode} CPU transform: {error}"));
        let actual = match transformed.use_backend(Backend::Gpu).tobytes() {
            Ok(actual) => actual,
            Err(error)
                if error.to_string().contains("GPU adapter not available")
                    || error
                        .to_string()
                        .contains("GPU device initialization failed") =>
            {
                Backend::set_pipeline_telemetry_enabled(previous);
                return;
            }
            Err(error) => panic!("native GPU {mode} transform failed: {error}"),
        };
        assert_eq!(actual, expected, "native GPU {mode} affine parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .unwrap_or_else(|| panic!("native GPU {mode} transform missing telemetry"));
        assert_eq!(telemetry.0, Some(Backend::Gpu), "{mode} requested backend");
        assert_eq!(telemetry.1, Backend::Gpu, "{mode} actual backend");
        assert_eq!(telemetry.7, None, "{mode} fallback reason");

        assert_typed_luma16_nearest_proof(&source, mode);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

fn assert_typed_luma16_nearest_proof(source: &Image, mode: &str) {
    let op = PipelineOp::Transform {
        w: 3,
        h: 2,
        method: TransformMethod::Affine,
        data: Arc::from(vec![0.5, 0.0, 0.0, 0.0, 0.5, 0.0]),
        filter: ResampleFilter::Bicubic,
        fill: Some((2, 1, 0, 0)),
        fill_is_none: false,
        palette_fill: None,
    };
    let image = source.materialize().expect("materialize typed source");
    assert!(
        gpu_nearest_affine_is_exact(&op, &image, Some(mode), (4, 3)),
        "{mode} nearest-affine proof"
    );
}

#[test]
fn f_resize_identity_lowering_copies_nonfinite_words() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            4,
            4,
            [
                f32::NAN.to_bits(),
                f32::INFINITY.to_bits(),
                (-0.0f32).to_bits(),
                (-1.25f32).to_bits(),
                0.0f32.to_bits(),
                1.0f32.to_bits(),
                2.0f32.to_bits(),
                3.0f32.to_bits(),
                4.0f32.to_bits(),
                5.0f32.to_bits(),
                6.0f32.to_bits(),
                7.0f32.to_bits(),
                8.0f32.to_bits(),
                9.0f32.to_bits(),
                10.0f32.to_bits(),
                11.0f32.to_bits(),
            ]
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect(),
        )
        .unwrap(),
    );
    let same_size = PipelineOp::Resize {
        w: 4,
        h: 4,
        filter: ResampleFilter::Bicubic,
    };
    assert_f_resize_identity_proofs(&image, &same_size);
}

fn assert_f_resize_identity_proofs(image: &DynamicImage, same_size: &PipelineOp) {
    assert!(gpu_f_resize_identity_is_exact(
        std::slice::from_ref(same_size),
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_identity_is_exact(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::from([0xde, 0xad, 0xbe, 0xef].repeat(16)),
                mode: PixelMode::F,
            },
            (*same_size).clone(),
        ],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_identity_is_exact(
        &[PipelineOp::Resize {
            w: 3,
            h: 4,
            filter: ResampleFilter::Bilinear,
        }],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_identity_is_exact(
        &[(*same_size).clone(), PipelineOp::Mirror],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_identity_is_exact(
        std::slice::from_ref(same_size),
        image,
        Some("I")
    ));
}

#[test]
fn luma16_filtered_resize_proof_covers_declared_byte_orders() {
    let values = [
        1u16, 258, 32_767, 65_535, 32_768, 43_981, 12_345, 54_321, 999, 40_000, 22_222, 65_534,
    ];
    let op = PipelineOp::Resize {
        w: 2,
        h: 2,
        filter: ResampleFilter::Bilinear,
    };
    for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
        let big_endian = mode == "I;16B";
        let bytes = values
            .iter()
            .flat_map(|value| {
                if big_endian {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                }
            })
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, (4, 3), &bytes).expect("I;16 source");
        let image = source.materialize().expect("materialize I;16 source");
        assert!(
            gpu_luma16_resize_f64_is_exact(std::slice::from_ref(&op), &image, Some(mode)),
            "filtered I;16 proof should cover {mode}"
        );
    }
}

#[test]
fn luma16_filtered_resize_native_gpu_matches_cpu() {
    let values = [
        1u16, 258, 32_767, 65_535, 32_768, 43_981, 12_345, 54_321, 999, 40_000, 22_222, 65_534, 7,
        511, 4096, 60000,
    ];
    let cases = [
        ((2u32, 2u32), ResampleFilter::Bilinear),
        ((7, 4), ResampleFilter::Lanczos),
        ((4, 5), ResampleFilter::Lanczos),
        ((5, 3), ResampleFilter::Box),
        ((3, 6), ResampleFilter::Hamming),
        ((6, 3), ResampleFilter::Bicubic),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
        let big_endian = mode == "I;16B";
        let bytes = values
            .iter()
            .flat_map(|value| {
                if big_endian {
                    value.to_be_bytes()
                } else {
                    value.to_le_bytes()
                }
            })
            .collect::<Vec<_>>();
        let source = Image::frombytes(mode, (4, 4), &bytes).expect("I;16 source");
        assert_eq!(source.mode().expect("I;16 mode"), mode);
        assert_eq!(
            luma16_resample_big_endian(Some(mode)),
            matches!(mode, "I;16B" | "I;16N")
        );
        for &((width, height), filter) in &cases {
            let filter_name = ResampleInput::Name(format!("{filter:?}").to_uppercase());
            let expected = source
                .resize(
                    (width as i64, height as i64),
                    Some(filter_name.clone()),
                    None,
                )
                .expect("CPU I;16 resize")
                .use_backend(Backend::Cpu)
                .tobytes()
                .expect("CPU I;16 bytes");
            let actual = match source
                .resize((width as i64, height as i64), Some(filter_name), None)
                .expect("GPU I;16 resize")
                .use_backend(Backend::Gpu)
                .tobytes()
            {
                Ok(actual) => actual,
                Err(error)
                    if error.to_string().contains("GPU adapter not available")
                        || error
                            .to_string()
                            .contains("GPU device initialization failed") =>
                {
                    Backend::set_pipeline_telemetry_enabled(previous);
                    return;
                }
                Err(error) => panic!("native GPU {mode} resize failed: {error}"),
            };
            assert_eq!(actual, expected, "native GPU I;16 {mode} {filter:?} resize");
            let telemetry = Backend::take_pipeline_telemetry()
                .expect("native GPU I;16 resize must publish a receipt");
            assert_eq!(telemetry.0, Some(Backend::Gpu));
            assert_eq!(telemetry.1, Backend::Gpu);
            assert_eq!(telemetry.6, Some(2));
            assert_eq!(telemetry.7, None);
        }
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn rgb_a_fit_uses_native_raw_channel_resize() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let mut source_bytes = vec![0u8; 9 * 8 * 4];
    source_bytes[(3 * 9 + 2) * 4..][..4].copy_from_slice(&[200, 100, 50, 128]);
    let source = Image::frombytes("RGBa", (9, 8), &source_bytes).expect("RGBa source");
    let fitted = crate::ops::imageops::fit(&source, 4, 3, Some("BILINEAR"), 0.0, (0.5, 0.5))
        .expect("RGBa Fit operation");
    let actual = match fitted.use_backend(Backend::Gpu).tobytes() {
        Ok(actual) => actual,
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            Backend::set_pipeline_telemetry_enabled(previous);
            return;
        }
        Err(error) => panic!("native GPU RGBa Fit failed: {error}"),
    };
    assert_eq!(
        actual,
        vec![
            4, 2, 1, 3, 5, 3, 1, 3, 0, 0, 0, 0, 0, 0, 0, 0, 14, 7, 3, 9, 19, 9, 5, 12, 0, 0, 0, 0,
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]
    );
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU RGBa Fit must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(2));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}
