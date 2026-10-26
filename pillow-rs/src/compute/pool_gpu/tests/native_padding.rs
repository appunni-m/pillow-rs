use super::*;

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_pad_admits_identity_and_aligned_native_l_resize() {
    let luma =
        DynamicImage::ImageLuma8(GrayImage::from_raw(3, 2, vec![11, 22, 33, 44, 55, 66]).unwrap());
    let identity = PipelineOp::Pad {
        w: 3,
        h: 5,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 0, 0, 255)),
        centering: (0.5, 0.5),
    };
    assert_gpu_packed_luma_identity_pad_admission(&identity, &luma);

    let aligned_luma = DynamicImage::ImageLuma8(
        GrayImage::from_raw(8, 6, (0..48).map(|sample| sample as u8).collect()).unwrap(),
    );
    let aligned_resize = PipelineOp::Pad {
        w: 4,
        h: 5,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 0, 0, 255)),
        centering: (0.5, 0.5),
    };
    assert_gpu_packed_luma_aligned_resize_pad_admission(&aligned_resize, &aligned_luma);

    let resized = PipelineOp::Pad {
        w: 5,
        h: 4,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 0, 0, 255)),
        centering: (0.5, 0.5),
    };
    assert_gpu_packed_luma_pad_rejections(&identity, &resized, &luma);
}

#[cfg(target_endian = "little")]
fn assert_gpu_packed_luma_identity_pad_admission(pad: &PipelineOp, luma: &DynamicImage) {
    assert!(super::gpu_packed_luma_pad_input(
        std::slice::from_ref(pad),
        luma,
        None
    ));
    assert!(super::gpu_packed_luma_pad_input(
        std::slice::from_ref(pad),
        luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_pad_input(
        std::slice::from_ref(pad),
        luma,
        Some("RGB")
    ));
    assert!(!super::gpu_packed_luma_pad_input(
        &[pad.clone(), PipelineOp::Duplicate],
        luma,
        Some("L")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_packed_luma_aligned_resize_pad_admission(
    pad: &PipelineOp,
    aligned_luma: &DynamicImage,
) {
    assert!(super::gpu_packed_luma_pad_input(
        std::slice::from_ref(pad),
        aligned_luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_pad_dispatch_supported(
        pad,
        aligned_luma.dimensions(),
        65_535
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_packed_luma_pad_rejections(
    identity: &PipelineOp,
    resized: &PipelineOp,
    luma: &DynamicImage,
) {
    assert!(!super::gpu_packed_luma_pad_input(
        std::slice::from_ref(resized),
        luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_pad_input(
        std::slice::from_ref(identity),
        &DynamicImage::ImageRgb8(RgbImage::from_raw(3, 2, vec![11; 18]).unwrap()),
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_pad_input(
        std::slice::from_ref(identity),
        &DynamicImage::ImageLuma8(GrayImage::new(0, 2)),
        Some("L")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_pad_admits_only_identity_contain() {
    let image = DynamicImage::ImageLumaA8(
        GrayAlphaImage::from_raw(
            3,
            2,
            vec![11, 211, 22, 222, 33, 233, 44, 244, 55, 255, 66, 166],
        )
        .unwrap(),
    );
    let pad = PipelineOp::Pad {
        w: 3,
        h: 3,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 0, 0, 47)),
        centering: (0.5, 1.0),
    };
    assert!(super::gpu_packed_la_pad_input(
        std::slice::from_ref(&pad),
        &image,
        Some("LA")
    ));
    assert!(super::gpu_packed_la_pad_input(
        std::slice::from_ref(&pad),
        &image,
        None
    ));
    assert!(!super::gpu_packed_la_pad_input(
        std::slice::from_ref(&pad),
        &image,
        Some("PA")
    ));
    assert!(super::gpu_packed_la_pad_dispatch_supported(
        &pad,
        (3, 2),
        65_535
    ));
    assert!(!super::gpu_packed_la_pad_dispatch_supported(
        &pad,
        (3, 2),
        0
    ));

    let resize = PipelineOp::Pad {
        w: 2,
        h: 2,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 0, 0, 47)),
        centering: (0.5, 1.0),
    };
    assert!(!super::gpu_packed_la_pad_input(
        std::slice::from_ref(&resize),
        &image,
        Some("LA")
    ));
    assert!(!super::gpu_packed_la_pad_input(
        &[pad.clone(), PipelineOp::Duplicate],
        &image,
        Some("LA")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_hsv_pad_plan_requires_aligned_identity_contain() {
    let image = DynamicImage::ImageRgb8(
        RgbImage::from_raw(8, 2, (0..48).map(|sample| sample as u8).collect()).unwrap(),
    );
    let pad = PipelineOp::Pad {
        w: 8,
        h: 5,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 19, 47, 11)),
        centering: (0.5, 0.5),
    };
    let dispatch = super::plan_native_hsv_pad_dispatch(
        std::slice::from_ref(&pad),
        &image,
        Some("HSV"),
        65_535,
        u32::MAX,
        u64::MAX,
    )
    .expect("aligned identity-contain HSV Pad must plan");
    assert_eq!(dispatch.input_bytes, 48);
    assert_eq!(dispatch.input_transfer_bytes, 48);
    assert_eq!(dispatch.output.transfer_bytes, 120);
    assert_eq!(dispatch.output_width, 8);
    assert_eq!(dispatch.output_height, 5);
    assert_eq!(dispatch.offset_y, 2);

    assert!(
        super::plan_native_hsv_pad_dispatch(
            std::slice::from_ref(&pad),
            &image,
            Some("RGB"),
            65_535,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(
        super::plan_native_hsv_pad_dispatch(
            &[pad.clone(), PipelineOp::Duplicate],
            &image,
            Some("HSV"),
            65_535,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );

    let unaligned = DynamicImage::ImageRgb8(
        RgbImage::from_raw(6, 2, (0..36).map(|sample| sample as u8).collect()).unwrap(),
    );
    let unaligned_pad = PipelineOp::Pad {
        w: 6,
        h: 5,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 19, 47, 11)),
        centering: (0.5, 0.5),
    };
    assert!(
        super::plan_native_hsv_pad_dispatch(
            std::slice::from_ref(&unaligned_pad),
            &unaligned,
            Some("HSV"),
            65_535,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
    assert!(
        super::plan_native_hsv_pad_dispatch(
            std::slice::from_ref(&pad),
            &image,
            Some("HSV"),
            0,
            u32::MAX,
            u64::MAX,
        )
        .is_none()
    );
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_hsv_pad_preserves_triplets_and_uses_compact_transfers() {
    use crate::compute::{execute_prepared, prepare_execution};

    let source = DynamicImage::ImageRgb8(
        RgbImage::from_raw(
            8,
            2,
            (0..48)
                .map(|index| ((index * 37 + index / 5 * 19 + 11) % 256) as u8)
                .collect(),
        )
        .expect("HSV source dimensions must be valid"),
    );
    let op = [PipelineOp::Pad {
        w: 8,
        h: 5,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 19, 47, 11)),
        centering: (0.5, 0.5),
    }];
    let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("HSV"))
        .expect("CPU HSV Pad reference");
    let prepared = prepare_execution(&op, Some(Backend::Gpu)).expect("GPU HSV Pad routing");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match execute_prepared(&prepared, &op, &source, Some("HSV")) {
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
        Err(error) => panic!("native GPU HSV Pad failed: {error}"),
    };
    assert!(matches!(&actual, DynamicImage::ImageRgb8(_)));
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    let receipt = Backend::take_pipeline_telemetry().expect("native HSV Pad receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("native HSV Pad resources");
    assert_eq!(resources.upload_bytes, 48);
    assert_eq!(resources.readback_bytes, 120);
    assert_eq!(resources.mode_conversion_count, 0);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_la_pad_preserves_odd_rows_fill_and_alpha() {
    use crate::compute::{execute_prepared, prepare_execution};

    let source = DynamicImage::ImageLumaA8(
        GrayAlphaImage::from_raw(
            3,
            2,
            vec![11, 211, 22, 222, 33, 233, 44, 244, 55, 255, 66, 166],
        )
        .unwrap(),
    );
    let op = [PipelineOp::Pad {
        w: 3,
        h: 3,
        filter: ResampleFilter::Lanczos,
        color: Some((173, 0, 0, 47)),
        centering: (0.5, 1.0),
    }];
    let expected = crate::compute::registry::execute_cpu(&op[0], &source, Some("LA"))
        .expect("CPU LA Pad reference");
    let prepared = prepare_execution(&op, Some(Backend::Gpu)).expect("GPU LA Pad routing");
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match execute_prepared(&prepared, &op, &source, Some("LA")) {
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
        Err(error) => panic!("native GPU LA Pad failed: {error}"),
    };
    assert!(matches!(&actual, DynamicImage::ImageLumaA8(_)));
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    let receipt = Backend::take_pipeline_telemetry().expect("native LA Pad receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("native LA Pad resources");
    assert_eq!(resources.upload_bytes, 12);
    assert_eq!(resources.readback_bytes, 20);
    assert_eq!(resources.mode_conversion_count, 0);
    Backend::set_pipeline_telemetry_enabled(previous);
}
