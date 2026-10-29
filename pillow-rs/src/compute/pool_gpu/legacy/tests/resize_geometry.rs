use super::*;

#[test]
fn f_pad_constant_source_preserves_scalar_fill_on_gpu() {
    // `ImageOps.pad` keeps F samples as opaque four-byte words through
    // the contain resize and final placement.  This constant source is
    // eligible for the exact scalar resize marker; the named fill is
    // resolved to Pillow's F luma value (76.0) before it reaches the
    // raw-word placement shader.
    let source = Image::new(16, 16, "F", (0, 0, 0, 0)).expect("F source");
    let expected = crate::ops::imageops::pad_with_input(
        &source,
        20,
        12,
        None,
        crate::ops::imageops::ImageOpsColor::Name("red".into()),
        crate::ops::imageops::CenteringInput::Default,
    )
    .expect("CPU pad operation")
    .use_backend(Backend::Cpu)
    .tobytes()
    .expect("CPU F pad");

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let actual = match crate::ops::imageops::pad_with_input(
        &source,
        20,
        12,
        None,
        crate::ops::imageops::ImageOpsColor::Name("red".into()),
        crate::ops::imageops::CenteringInput::Default,
    )
    .expect("GPU pad operation")
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
        Err(error) => panic!("native GPU F pad failed: {error}"),
    };
    assert_eq!(actual, expected, "native GPU F pad scalar fill");
    let telemetry = Backend::take_pipeline_telemetry()
        .expect("native GPU F pad must publish a telemetry receipt");
    assert_eq!(telemetry.0, Some(Backend::Gpu));
    assert_eq!(telemetry.1, Backend::Gpu);
    assert_eq!(telemetry.6, Some(3));
    assert_eq!(telemetry.7, None);
    Backend::set_pipeline_telemetry_enabled(previous);
}

#[test]
fn pad_geometry_rejects_rounded_zero_contain_axes() {
    for (source_dimensions, target_dimensions) in [((2, 100), (1, 1)), ((100, 2), (1, 1))] {
        let op = PipelineOp::Pad {
            w: target_dimensions.0,
            h: target_dimensions.1,
            filter: ResampleFilter::Box,
            color: None,
            centering: (0.5, 0.5),
        };
        assert_eq!(
            gpu_pad_geometry(&op, source_dimensions.0, source_dimensions.1),
            None
        );
    }
}

#[test]
fn boxed_nearest_resize_dispatch_count_covers_horizontal_and_vertical_passes() {
    for mode in ["L", "LA", "RGB", "RGBA"] {
        let boxed_nearest = PipelineOp::ResizeBoxed {
            w: 5,
            h: 3,
            filter: ResampleFilter::Nearest,
            box_coords: (0.25, 1.5, 1.75, 202.0),
        };
        assert_eq!(
            gpu_dispatch_count(std::slice::from_ref(&boxed_nearest), Some(mode), (2, 203)),
            2,
            "{mode} ResizeBoxed nearest must account for H and V dispatches"
        );
    }
}

#[test]
fn pad_dispatch_count_skips_identity_contain_resize() {
    let identity_resize = PipelineOp::Pad {
        w: 1024,
        h: 1024,
        filter: ResampleFilter::Lanczos,
        color: Some((17, 83, 149, 31)),
        centering: (0.5, 0.5),
    };
    assert_eq!(
        gpu_dispatch_count(
            std::slice::from_ref(&identity_resize),
            Some("CMYK"),
            (1024, 768)
        ),
        1
    );

    let resized = PipelineOp::Pad {
        w: 1200,
        h: 900,
        filter: ResampleFilter::Lanczos,
        color: Some((17, 83, 149, 31)),
        centering: (0.5, 0.5),
    };
    assert_eq!(
        gpu_dispatch_count(std::slice::from_ref(&resized), Some("CMYK"), (1024, 768)),
        // The contain dimensions already equal the destination canvas,
        // so only the horizontal and vertical resize passes are emitted;
        // the placement pass is an identity and is skipped.
        2
    );
}

#[test]
fn rgbx_vertical_pad_fusion_is_limited_to_full_width_filtered_resize() {
    let resized = PipelineOp::Pad {
        w: 768,
        h: 768,
        filter: ResampleFilter::Lanczos,
        color: Some((17, 83, 149, 31)),
        centering: (0.5, 0.5),
    };
    assert_eq!(
        gpu_pad_rgbx_fused_vertical_geometry(&resized, (1024, 768), Some("RGBX")),
        Some(((768, 576), (0, 96)))
    );
    assert_eq!(
        gpu_dispatch_count(std::slice::from_ref(&resized), Some("RGBX"), (1024, 768)),
        2
    );
    assert_eq!(
        gpu_shader_work_items(&resized, (1024, 768), (768, 768), Some("RGBX")),
        Some(1_179_648)
    );

    assert_eq!(
        gpu_pad_rgbx_fused_vertical_geometry(&resized, (1024, 768), Some("CMYK")),
        None
    );
    assert_eq!(
        gpu_dispatch_count(std::slice::from_ref(&resized), Some("CMYK"), (1024, 768)),
        3
    );

    let nearest = PipelineOp::Pad {
        w: 768,
        h: 768,
        filter: ResampleFilter::Nearest,
        color: Some((17, 83, 149, 31)),
        centering: (0.5, 0.5),
    };
    assert_eq!(
        gpu_pad_rgbx_fused_vertical_geometry(&nearest, (1024, 768), Some("RGBX")),
        None
    );

    let horizontal_padding = PipelineOp::Pad {
        w: 1200,
        h: 768,
        filter: ResampleFilter::Lanczos,
        color: Some((17, 83, 149, 31)),
        centering: (0.5, 0.5),
    };
    assert_eq!(
        gpu_pad_rgbx_fused_vertical_geometry(&horizontal_padding, (1024, 768), Some("RGBX")),
        None
    );
}

#[test]
fn gpu_pad_fill_keeps_rgbx_and_cmyk_fourth_byte_semantics() {
    let default_fill = PipelineOp::Pad {
        w: 2,
        h: 3,
        filter: ResampleFilter::Bicubic,
        color: None,
        centering: (0.5, 0.5),
    };
    assert_eq!(gpu_pad_fill(&default_fill, Some("RGBX"), 6), 0);
    assert_eq!(gpu_pad_fill(&default_fill, Some("CMYK"), 4), 0);

    let explicit_fill = PipelineOp::Pad {
        w: 2,
        h: 3,
        filter: ResampleFilter::Bicubic,
        color: Some((17, 83, 149, 31)),
        centering: (0.5, 0.5),
    };
    assert_eq!(
        gpu_pad_fill(&explicit_fill, Some("RGBX"), 6),
        17 | (83 << 8) | (149 << 16) | (31 << 24)
    );
}

#[test]
fn alpha_composite_mirror_fuses_only_matching_full_frame_rgba() {
    let source = Arc::new(
        crate::image::Image::new(2, 1, "RGBA", (0, 0, 0, 0)).expect("RGBA composite source"),
    );
    let pair = [
        PipelineOp::AlphaComposite {
            source: Arc::clone(&source),
            dest: (0, 0),
            src: (0, 0),
        },
        PipelineOp::Mirror,
    ];
    assert!(super::can_fuse_gpu_alpha_composite_mirror(
        &pair,
        0,
        Some("RGBA"),
        (2, 1),
    ));
    assert_eq!(super::gpu_dispatch_count(&pair, Some("RGBA"), (2, 1)), 1);
    assert_eq!(
        super::gpu_fused_operation_count(&pair, Some("RGBA"), (2, 1)),
        2
    );
    assert!(!super::can_fuse_gpu_alpha_composite_mirror(
        &pair,
        0,
        Some("RGB"),
        (2, 1),
    ));

    let offset_pair = [
        PipelineOp::AlphaComposite {
            source,
            dest: (1, 0),
            src: (0, 0),
        },
        PipelineOp::Mirror,
    ];
    assert!(!super::can_fuse_gpu_alpha_composite_mirror(
        &offset_pair,
        0,
        Some("RGBA"),
        (2, 1),
    ));

    let wrong_size_source = Arc::new(
        crate::image::Image::new(1, 1, "RGBA", (0, 0, 0, 0))
            .expect("mismatched RGBA composite source"),
    );
    let wrong_size_pair = [
        PipelineOp::AlphaComposite {
            source: wrong_size_source,
            dest: (0, 0),
            src: (0, 0),
        },
        PipelineOp::Mirror,
    ];
    assert!(!super::can_fuse_gpu_alpha_composite_mirror(
        &wrong_size_pair,
        0,
        Some("RGBA"),
        (2, 1),
    ));
}
