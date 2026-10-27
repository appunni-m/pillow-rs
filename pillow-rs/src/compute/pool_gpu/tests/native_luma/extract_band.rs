use crate::{Backend, Image, PipelineOp};

type ExtractBandCase = (&'static str, i32, Vec<u8>, Vec<u8>);

fn native_extract_band_cases() -> [ExtractBandCase; 12] {
    [
        ("L", 0, vec![71, 19], vec![71, 19]),
        ("LA", 1, vec![10, 201, 20, 202], vec![201, 202]),
        ("RGB", 0, vec![1, 2, 3, 11, 12, 13], vec![1, 11]),
        ("RGB", 1, vec![1, 2, 3, 11, 12, 13], vec![2, 12]),
        ("RGB", 2, vec![1, 2, 3, 11, 12, 13], vec![3, 13]),
        (
            "RGBA",
            0,
            vec![21, 22, 23, 24, 31, 32, 33, 34],
            vec![21, 31],
        ),
        (
            "RGBA",
            1,
            vec![21, 22, 23, 24, 31, 32, 33, 34],
            vec![22, 32],
        ),
        (
            "RGBA",
            2,
            vec![21, 22, 23, 24, 31, 32, 33, 34],
            vec![23, 33],
        ),
        (
            "RGBA",
            3,
            vec![21, 22, 23, 24, 31, 32, 33, 34],
            vec![24, 34],
        ),
        ("CMYK", 3, vec![1, 2, 3, 4, 11, 12, 13, 14], vec![4, 14]),
        ("HSV", 1, vec![21, 22, 23, 31, 32, 33], vec![22, 32]),
        ("YCbCr", 2, vec![41, 42, 43, 51, 52, 53], vec![43, 53]),
    ]
}

#[test]
fn extract_band_native_gpu_preserves_raw_color_mode_channels() {
    let cases = native_extract_band_cases();
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    for (mode, channel, source_bytes, expected_bytes) in cases {
        let source = Image::frombytes(mode, (2, 1), &source_bytes).expect("source image");
        let expected = source
            .getchannel(channel)
            .expect("channel operation")
            .use_backend(Backend::Cpu)
            .tobytes()
            .expect("CPU channel extraction");
        assert_eq!(expected, expected_bytes);
        let _ = crate::compute::take_pipeline_resource_telemetry();
        // `use_backend` only locks deferred work. The public getchannel
        // fast paths for single-band L and tiny RGB can execute eagerly
        // before a lock on the returned image is applied, so construct
        // the same ExtractBand pipeline directly for this GPU-kernel test.
        let actual = match Image::push_op(
            &source,
            PipelineOp::ExtractBand {
                index: u8::try_from(channel).expect("valid channel index"),
            },
        )
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
            Err(error) => panic!("native GPU {mode} channel extraction failed: {error}"),
        };
        assert_eq!(actual, expected, "{mode} channel extraction parity");
        let telemetry = Backend::take_pipeline_telemetry()
            .expect("native GPU channel extraction must publish a receipt");
        assert_eq!(telemetry.0, Some(Backend::Gpu));
        assert_eq!(telemetry.1, Backend::Gpu);
        assert_eq!(telemetry.6, Some(1));
        assert_eq!(telemetry.7, None);
        let resources = telemetry
            .8
            .expect("native GPU channel extraction must publish resource telemetry");
        assert_eq!(
            resources.upload_bytes,
            source_bytes.len().div_ceil(4) as u64 * 4,
            "{mode} upload must keep its native byte layout"
        );
        assert_eq!(resources.readback_bytes, 4);
        assert_eq!(resources.mode_conversion_count, 0);
    }
    Backend::set_pipeline_telemetry_enabled(previous);
}
