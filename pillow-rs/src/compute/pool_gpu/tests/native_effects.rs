use super::*;

#[test]
#[cfg(target_endian = "little")]
fn luma_equalize_packed_histogram_matches_cpu_for_every_luma_tail() {
    use crate::compute::BackendImpl;

    struct RestoreTelemetry(bool);
    impl Drop for RestoreTelemetry {
        fn drop(&mut self) {
            Backend::set_pipeline_telemetry_enabled(self.0);
        }
    }
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let _restore_telemetry = RestoreTelemetry(previous);

    match super::GpuPool::ensure_init() {
        Ok(_) => {}
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            return;
        }
        Err(error) => panic!("GPU L Equalize initialization failed: {error}"),
    }

    // Cover Pillow's zero-step boundary, all packed-word tails, and a
    // multi-row input whose final packed word crosses a row boundary.
    for (width, height) in [
        (255u32, 1u32),
        (256, 1),
        (257, 1),
        (258, 1),
        (259, 1),
        (260, 1),
        (31, 17),
    ] {
        let pixel_count = width as usize * height as usize;
        let bytes: Vec<u8> = (0..pixel_count).map(|pixel| pixel as u8).collect();
        let source = DynamicImage::ImageLuma8(
            GrayImage::from_raw(width, height, bytes).expect("L Equalize source"),
        );
        let op = PipelineOp::Equalize;
        let expected = crate::compute::registry::execute_cpu(&op, &source, Some("L"))
            .unwrap_or_else(|error| panic!("CPU L Equalize {width}x{height}: {error}"));
        let actual = super::GpuPool
            .execute_batch_strict(std::slice::from_ref(&op), &source, Some("L"))
            .unwrap_or_else(|error| panic!("GPU L Equalize {width}x{height}: {error}"));
        assert!(
            matches!(&actual, DynamicImage::ImageLuma8(_)),
            "{width}x{height} must remain native L"
        );
        assert_eq!(actual.color(), expected.color(), "{width}x{height} mode");
        assert_eq!(
            actual.as_bytes(),
            expected.as_bytes(),
            "{width}x{height} pixels"
        );

        let resources = crate::compute::take_pipeline_resource_telemetry()
            .unwrap_or_else(|| panic!("{width}x{height} resource receipt"));
        let transfer_bytes = pixel_count.div_ceil(4) * 4;
        assert_eq!(
            resources.upload_bytes, transfer_bytes as u64,
            "{width}x{height} packed L upload"
        );
        assert_eq!(
            resources.readback_bytes, transfer_bytes as u64,
            "{width}x{height} packed L readback"
        );
        assert_eq!(
            resources.mode_conversion_count, 0,
            "{width}x{height} must not widen L"
        );
        assert_eq!(
            crate::compute::take_pipeline_dispatch_count(),
            Some(3),
            "{width}x{height} compute dispatches"
        );
        assert_eq!(
            crate::compute::take_pipeline_backend_override(),
            None,
            "{width}x{height} must execute on GPU without fallback"
        );
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_sharpness_rgb_requires_singleton_rgb_storage() {
    let sharpness = PipelineOp::Sharpness { factor: 1.5 };
    let rgb = DynamicImage::ImageRgb8(
        RgbImage::from_raw(3, 1, vec![12, 34, 56, 78, 90, 123, 234, 210, 98]).unwrap(),
    );
    assert!(super::gpu_native_sharpness_rgb_input(
        std::slice::from_ref(&sharpness),
        &rgb,
        None
    ));
    assert!(super::gpu_native_sharpness_rgb_input(
        std::slice::from_ref(&sharpness),
        &rgb,
        Some("RGB")
    ));
    assert!(!super::gpu_native_sharpness_rgb_input(
        std::slice::from_ref(&sharpness),
        &rgb,
        Some("HSV")
    ));
    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            3,
            1,
            vec![12, 34, 56, 78, 90, 123, 234, 210, 98, 76, 54, 32],
        )
        .unwrap(),
    );
    assert!(!super::gpu_native_sharpness_rgb_input(
        std::slice::from_ref(&sharpness),
        &rgba,
        Some("RGB")
    ));
    let multiple = [sharpness, PipelineOp::Duplicate];
    assert!(!super::gpu_native_sharpness_rgb_input(
        &multiple,
        &rgb,
        Some("RGB")
    ));
    let empty = DynamicImage::ImageRgb8(RgbImage::new(0, 1));
    assert!(!super::gpu_native_sharpness_rgb_input(
        &[PipelineOp::Sharpness { factor: 1.5 }],
        &empty,
        Some("RGB")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_sharpness_l_requires_singleton_l_storage() {
    let sharpness = PipelineOp::Sharpness { factor: 1.5 };
    let luma =
        DynamicImage::ImageLuma8(GrayImage::from_raw(3, 2, vec![12, 34, 56, 78, 90, 123]).unwrap());
    assert!(super::gpu_native_sharpness_l_input(
        std::slice::from_ref(&sharpness),
        &luma,
        None
    ));
    assert!(super::gpu_native_sharpness_l_input(
        std::slice::from_ref(&sharpness),
        &luma,
        Some("L")
    ));
    assert!(!super::gpu_native_sharpness_l_input(
        std::slice::from_ref(&sharpness),
        &luma,
        Some("LA")
    ));

    let multiple = [sharpness, PipelineOp::Duplicate];
    assert!(!super::gpu_native_sharpness_l_input(
        &multiple,
        &luma,
        Some("L")
    ));
    let rgb = DynamicImage::ImageRgb8(RgbImage::new(3, 2));
    assert!(!super::gpu_native_sharpness_l_input(
        &[PipelineOp::Sharpness { factor: 1.5 }],
        &rgb,
        Some("L")
    ));
    let empty = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert!(!super::gpu_native_sharpness_l_input(
        &[PipelineOp::Sharpness { factor: 1.5 }],
        &empty,
        Some("L")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_sharpness_la_requires_singleton_la_storage() {
    let la = DynamicImage::ImageLumaA8(
        GrayAlphaImage::from_raw(3, 1, vec![10, 20, 30, 40, 50, 60]).unwrap(),
    );
    assert_gpu_native_sharpness_la_modes(&la);

    let putpixel = PipelineOp::PutPixel {
        x: 1,
        y: 0,
        color: (77, 88, 99, 123),
        palette_index: false,
    };
    let write_then_sharpen = [putpixel, PipelineOp::Sharpness { factor: 1.5 }];
    assert!(super::gpu_native_sharpness_la_input(
        &write_then_sharpen,
        &la,
        Some("LA")
    ));
    assert_eq!(
        super::gpu_native_sharpness_la_putpixel(&write_then_sharpen, &la, Some("LA")),
        Some((2, [77, 123]))
    );
    let out_of_bounds = [
        PipelineOp::PutPixel {
            x: 3,
            y: 0,
            color: (77, 88, 99, 123),
            palette_index: false,
        },
        PipelineOp::Sharpness { factor: 1.5 },
    ];
    assert!(!super::gpu_native_sharpness_la_input(
        &out_of_bounds,
        &la,
        Some("LA")
    ));
    let empty = DynamicImage::ImageLumaA8(GrayAlphaImage::new(0, 1));
    assert!(!super::gpu_native_sharpness_la_input(
        &[PipelineOp::Sharpness { factor: 1.5 }],
        &empty,
        Some("LA")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_sharpness_la_modes(la: &DynamicImage) {
    let sharpness = PipelineOp::Sharpness { factor: 1.5 };
    assert!(super::gpu_native_sharpness_la_input(
        std::slice::from_ref(&sharpness),
        la,
        None
    ));
    assert!(super::gpu_native_sharpness_la_input(
        std::slice::from_ref(&sharpness),
        la,
        Some("LA")
    ));
    assert!(!super::gpu_native_sharpness_la_input(
        std::slice::from_ref(&sharpness),
        la,
        Some("La")
    ));
    assert!(!super::gpu_native_sharpness_la_input(
        std::slice::from_ref(&sharpness),
        la,
        Some("PA")
    ));

    let multiple = [sharpness, PipelineOp::Duplicate];
    assert!(!super::gpu_native_sharpness_la_input(
        &multiple,
        la,
        Some("LA")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn native_la_sharpness_readback_extracts_luma_and_alpha() {
    let output = super::la_from_rgba_transport(2, 1, &[10, 10, 10, 20, 30, 30, 30, 40]).unwrap();
    assert!(matches!(output, DynamicImage::ImageLumaA8(_)));
    assert_eq!(output.as_bytes(), [10, 20, 30, 40]);
    assert!(super::la_from_rgba_transport(2, 1, &[10, 10, 10, 20]).is_err());
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_reduce_rgb_requires_singleton_rgb_storage() {
    let reduce = PipelineOp::Reduce {
        x_factor: 3,
        y_factor: 5,
    };
    let rgb = DynamicImage::ImageRgb8(
        RgbImage::from_raw(3, 1, vec![12, 34, 56, 78, 90, 123, 234, 210, 98]).unwrap(),
    );
    assert!(super::gpu_native_reduce_rgb_input(
        std::slice::from_ref(&reduce),
        &rgb,
        None
    ));
    assert!(super::gpu_native_reduce_rgb_input(
        std::slice::from_ref(&reduce),
        &rgb,
        Some("RGB")
    ));
    assert!(!super::gpu_native_reduce_rgb_input(
        std::slice::from_ref(&reduce),
        &rgb,
        Some("HSV")
    ));
    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            3,
            1,
            vec![12, 34, 56, 78, 90, 123, 234, 210, 98, 76, 54, 32],
        )
        .unwrap(),
    );
    assert!(!super::gpu_native_reduce_rgb_input(
        std::slice::from_ref(&reduce),
        &rgba,
        Some("RGB")
    ));
    let multiple = [reduce, PipelineOp::Duplicate];
    assert!(!super::gpu_native_reduce_rgb_input(
        &multiple,
        &rgb,
        Some("RGB")
    ));
    let empty = DynamicImage::ImageRgb8(RgbImage::new(0, 1));
    assert!(!super::gpu_native_reduce_rgb_input(
        &[PipelineOp::Reduce {
            x_factor: 3,
            y_factor: 5,
        }],
        &empty,
        Some("RGB")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_posterize_requires_matching_l_or_rgb_storage() {
    let posterize = PipelineOp::Posterize { bits: 4 };
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(3, 1, vec![1, 127, 255]).unwrap());
    assert_eq!(
        super::gpu_native_byte_op_channels(&posterize, &luma, None),
        Some(1)
    );
    assert_eq!(
        super::gpu_native_byte_op_channels(&posterize, &luma, Some("L")),
        Some(1)
    );
    assert_eq!(
        super::gpu_native_byte_op_channels(&posterize, &luma, Some("P")),
        None
    );

    let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![1, 127, 255]).unwrap());
    assert_eq!(
        super::gpu_native_byte_op_channels(&posterize, &rgb, None),
        Some(3)
    );
    assert_eq!(
        super::gpu_native_byte_op_channels(&posterize, &rgb, Some("RGB")),
        Some(3)
    );
    assert_eq!(
        super::gpu_native_byte_op_channels(&posterize, &rgb, Some("HSV")),
        None
    );
    assert_eq!(
        super::gpu_native_byte_op_channels(
            &posterize,
            &DynamicImage::ImageRgba8(RgbaImage::new(1, 1)),
            Some("RGBA")
        ),
        None
    );
    assert_eq!(
        super::gpu_native_byte_op_channels(&PipelineOp::Posterize { bits: 0 }, &rgb, Some("RGB")),
        None
    );
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_duplicate_uses_native_storage_bytes_for_matching_modes() {
    use crate::pipeline::PipelineOp;
    use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

    let duplicate = PipelineOp::Duplicate;
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![17, 239]).unwrap());
    for mode in [None, Some("1"), Some("L"), Some("P")] {
        assert_eq!(
            super::gpu_native_byte_op_channels(&duplicate, &luma, mode),
            Some(1),
            "native L samples for mode {mode:?}"
        );
    }
    assert_eq!(
        super::gpu_native_byte_op_channels(&duplicate, &luma, Some("RGB")),
        None
    );

    let la = DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(1, 1, vec![29, 203]).unwrap());
    for mode in [None, Some("LA"), Some("PA")] {
        assert_eq!(
            super::gpu_native_byte_op_channels(&duplicate, &la, mode),
            Some(2),
            "native LA samples for mode {mode:?}"
        );
    }

    let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![3, 127, 251]).unwrap());
    for mode in [None, Some("RGB"), Some("HSV"), Some("YCbCr")] {
        assert_eq!(
            super::gpu_native_byte_op_channels(&duplicate, &rgb, mode),
            Some(3),
            "native RGB samples for mode {mode:?}"
        );
    }

    let rgba = DynamicImage::ImageRgba8(RgbaImage::from_raw(1, 1, vec![11, 77, 149, 221]).unwrap());
    for mode in [
        None,
        Some("RGBA"),
        Some("CMYK"),
        Some("RGBa"),
        Some("RGBX"),
        Some("I"),
        Some("F"),
    ] {
        assert_eq!(
            super::gpu_native_byte_op_channels(&duplicate, &rgba, mode),
            Some(4),
            "native four-byte samples for mode {mode:?}"
        );
    }

    let empty = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert_eq!(
        super::gpu_native_byte_op_channels(&duplicate, &empty, Some("L")),
        None,
        "empty storage cannot produce a zero-width workgroup grid"
    );
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_invert_keeps_palette_indices_in_one_byte() {
    let palette = DynamicImage::ImageLuma8(GrayImage::from_raw(3, 1, vec![1, 127, 255]).unwrap());
    for op in [PipelineOp::Invert, PipelineOp::InvertChops] {
        assert_eq!(
            super::gpu_native_byte_op_channels(&op, &palette, Some("P")),
            Some(1)
        );
    }
    assert_eq!(
        super::gpu_native_byte_op_channels(&PipelineOp::Invert, &palette, Some("L")),
        Some(1)
    );
    assert_eq!(
        super::gpu_native_byte_op_channels(&PipelineOp::InvertChops, &palette, Some("L")),
        None
    );

    let empty_palette = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert_eq!(
        super::gpu_native_byte_op_channels(&PipelineOp::InvertChops, &empty_palette, Some("P")),
        None
    );
}
