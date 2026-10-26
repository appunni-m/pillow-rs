use super::*;

fn f32_words_to_bytes(words: &[u32]) -> Vec<u8> {
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

#[test]
fn f_resize_compact_box_proof_covers_horizontal_over_binding_row() {
    let height = 8_388_608usize;
    let pattern = [0.125f32, -17.25, 1.0, -2.5, 3.75, -0.0, 100.0, -99.0];
    let words: Vec<u32> = (0..height)
        .map(|index| pattern[index % pattern.len()].to_bits())
        .collect();
    let source = Image::frombytes("F", (height as u32, 1), &f32_words_to_bytes(&words))
        .expect("horizontal compact Box source");
    let source_dynamic = source
        .materialize()
        .expect("materialize horizontal compact Box source");
    let op = PipelineOp::Resize {
        w: 1,
        h: 1,
        filter: ResampleFilter::Box,
    };
    assert_horizontal_over_binding_compact_box_proof(&op, &source_dynamic, height as u32);
}

fn assert_horizontal_over_binding_compact_box_proof(
    op: &PipelineOp,
    source: &DynamicImage,
    height: u32,
) {
    assert!(gpu_f_resize_compact_box_vertical_only_geometry(
        op,
        (1, height),
        (1, 1),
        Some("F")
    ));
    assert!(gpu_f_resize_compact_box_is_exact(
        std::slice::from_ref(op),
        source,
        Some("F")
    ));
    let compact_words =
        encode_resize_compact_box_axis(height, 1).expect("compact Box coefficient row");
    assert_eq!(compact_words.len(), 7);
    assert_eq!(&compact_words[0..3], &[0, height, 0]);
    assert!(!gpu_f_resize_f64_is_exact(
        std::slice::from_ref(op),
        source,
        Some("F")
    ));
    assert!(!gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(op),
        source,
        Some("F")
    ));
    assert!(gpu_f_resize_f64_ordered_is_exact(
        std::slice::from_ref(op),
        source,
        Some("F")
    ));
    assert!(!gpu_dimensions_require_cpu(
        std::slice::from_ref(op),
        source,
        Some("F")
    ));
    assert!(!gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(op),
        (1, height),
        65_535,
        Some("F"),
        false,
    ));
    assert_eq!(
        gpu_dispatch_count(std::slice::from_ref(op), Some("F"), (1, height)),
        1
    );
}

#[test]
fn f_resize_compact_box_proof_covers_multiple_output_rows() {
    // Two output columns each consume an integer 8,388,608-tap Box row.
    // The complete per-tap table would exceed the binding limit, while
    // Pillow's Resample.c geometry gives both rows the same 1/ratio
    // coefficient and contiguous source ranges.
    let ratio = 8_388_608usize;
    let width = ratio * 2;
    let pattern = [0.125f32, -17.25, 1.0, -2.5, 3.75, -0.0, 100.0, -99.0];
    let words: Vec<u32> = (0..width)
        .map(|index| pattern[index % pattern.len()].to_bits())
        .collect();
    let source = Image::frombytes("F", (width as u32, 1), &f32_words_to_bytes(&words))
        .expect("multi-output compact Box source");
    let source_dynamic = source
        .materialize()
        .expect("materialize multi-output compact Box source");
    let op = PipelineOp::Resize {
        w: 2,
        h: 1,
        filter: ResampleFilter::Box,
    };
    assert_multiple_output_compact_box_proof(&op, &source_dynamic, width as u32, ratio as u32);
    let filter = ResampleInput::Name("BOX".into());
    let expected = source
        .resize((2, 1), Some(filter.clone()), None)
        .expect("CPU multi-output compact Box resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU multi-output compact Box bytes");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = source
        .resize((2, 1), Some(filter), None)
        .expect("GPU multi-output compact Box resize")
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native GPU multi-output compact Box bytes");
    assert_eq!(actual, expected);
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("multi-output compact Box resize must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);

    assert!(!gpu_f_resize_compact_box_axis(width as u32 + 1, 2));
    assert!(encode_resize_compact_box_axis(width as u32 + 1, 2).is_err());
}

fn assert_multiple_output_compact_box_proof(
    op: &PipelineOp,
    source: &DynamicImage,
    width: u32,
    ratio: u32,
) {
    assert!(gpu_f_resize_compact_box_is_exact(
        std::slice::from_ref(op),
        source,
        Some("F")
    ));
    let compact_words = encode_resize_compact_box_axis(width, 2)
        .expect("multi-output compact Box coefficient rows");
    assert_eq!(compact_words.len(), 10);
    assert_eq!(&compact_words[0..6], &[0, ratio, 0, ratio, ratio, 0]);
    assert_eq!(
        &compact_words[6..],
        &encode_resize_compact_box_axis(ratio, 1)
            .expect("single-output compact Box coefficient row")[3..]
    );
    assert!(!gpu_dimensions_require_cpu(
        std::slice::from_ref(op),
        source,
        Some("F")
    ));
    assert!(!gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(op),
        (width, 1),
        65_535,
        Some("F"),
        false,
    ));
}

#[test]
fn f_resize_compact_box_proof_covers_vertical_over_binding_row() {
    let height = 8_388_608usize;
    let pattern = [0.125f32, -17.25, 1.0, -2.5, 3.75, -0.0, 100.0, -99.0];
    let words: Vec<u32> = (0..height)
        .map(|index| pattern[index % pattern.len()].to_bits())
        .collect();
    let source = Image::frombytes("F", (1, height as u32), &f32_words_to_bytes(&words))
        .expect("vertical compact Box source");
    let source_dynamic = source
        .materialize()
        .expect("materialize vertical compact Box source");
    let op = PipelineOp::Resize {
        w: 1,
        h: 1,
        filter: ResampleFilter::Box,
    };
    assert!(gpu_f_resize_compact_box_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    let compact_words =
        encode_resize_compact_box_axis(height as u32, 1).expect("compact Box coefficient row");
    assert_eq!(compact_words.len(), 7);
    assert_eq!(&compact_words[0..3], &[0, height as u32, 0]);
    assert!(!gpu_f_resize_f64_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(!gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(gpu_f_resize_f64_ordered_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
}

#[test]
fn f_resize_compact_box_proof_covers_changed_second_axis() {
    // The horizontal compact row is followed by a two-tap ordinary Box
    // row.  This exercises the pass ordering that was previously rejected
    // solely because the non-compact axis changed.
    let width = 8_388_608usize;
    let pattern = [0.125f32, -17.25, 1.0, -2.5, 3.75, -0.0, 100.0, -99.0];
    let words: Vec<u32> = (0..width * 2)
        .map(|index| pattern[index % pattern.len()].to_bits())
        .collect();
    let source = Image::frombytes("F", (width as u32, 2), &f32_words_to_bytes(&words))
        .expect("compact chained Box source");
    let source_dynamic = source
        .materialize()
        .expect("materialize compact chained Box source");
    let op = PipelineOp::Resize {
        w: 1,
        h: 1,
        filter: ResampleFilter::Box,
    };
    assert!(gpu_f_resize_compact_box_is_exact(
        std::slice::from_ref(&op),
        &source_dynamic,
        Some("F")
    ));
    assert!(!gpu_f_resize_compact_box_vertical_only_geometry(
        &op,
        (width as u32, 2),
        (1, 1),
        Some("F")
    ));
    let mut special_words = words.clone();
    special_words[width / 2] = 0x7fc1_2345;
    let special_source =
        Image::frombytes("F", (width as u32, 2), &f32_words_to_bytes(&special_words))
            .expect("compact chained special Box source")
            .materialize()
            .expect("materialize compact chained special Box source");
    assert!(!gpu_f_resize_compact_box_is_exact(
        std::slice::from_ref(&op),
        &special_source,
        Some("F")
    ));
    let filter = ResampleInput::Name("BOX".into());
    let expected = source
        .resize((1, 1), Some(filter.clone()), None)
        .expect("CPU compact chained Box resize")
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU compact chained Box bytes");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = source
        .resize((1, 1), Some(filter), None)
        .expect("GPU compact chained Box resize")
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native GPU compact chained Box bytes");
    assert_eq!(actual, expected);
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("compact chained Box resize must publish telemetry");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn f_resize_compact_box_special_over_binding_native_matches_cpu() {
    // Marker 13 repeats a nonzero 1/source-axis coefficient, so its
    // compact special scan can preserve the same first-NaN payload and
    // signed-infinity cancellation as marker 9 without a full table.
    // Exercise both shader directions at the adapter-fitting boundary.
    let width = 8_388_608usize;
    let cases = [
        (
            width as u32,
            1u32,
            vec![(width / 2, 0x7fa1_2345u32)],
            "horizontal NaN",
        ),
        (
            width as u32,
            1u32,
            vec![(width / 3, 0x7f80_0000u32), (width * 2 / 3, 0xff80_0000)],
            "horizontal opposite infinities",
        ),
        (
            1u32,
            width as u32,
            vec![(width / 3, 0x7f80_0000u32), (width * 2 / 3, 0xff80_0000)],
            "vertical opposite infinities",
        ),
        (
            1u32,
            width as u32,
            vec![(width / 2, 0x7fc2_3456u32)],
            "vertical NaN",
        ),
    ];
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (source_w, source_h, specials, label) in cases {
        let mut words = vec![0x3f80_0000u32; width];
        for (index, value) in specials {
            words[index] = value;
        }
        let source = Image::frombytes("F", (source_w, source_h), &f32_words_to_bytes(&words))
            .expect("compact special F source");
        let source_dynamic = source
            .materialize()
            .expect("materialize compact special F source");
        let op = PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Box,
        };
        assert!(
            gpu_f_resize_compact_box_is_exact(
                std::slice::from_ref(&op),
                &source_dynamic,
                Some("F")
            ),
            "compact proof should cover {label}"
        );
        let filter = ResampleInput::Name("BOX".into());
        let expected = source
            .resize((1, 1), Some(filter.clone()), None)
            .expect("CPU compact special F resize")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU compact special F bytes");
        let actual = match source
            .resize((1, 1), Some(filter), None)
            .expect("GPU compact special F resize")
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
            Err(error) => panic!("native GPU compact special F resize failed: {error}"),
        };
        assert_eq!(actual, expected, "native compact special {label}");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("compact special F resize must publish telemetry");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(
            telemetry.1,
            Backend::Gpu,
            "{label}; fallback reason: {:?}",
            telemetry.7
        );
        assert_eq!(telemetry.7, None);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}
