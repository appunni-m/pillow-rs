use super::*;

struct ProjectiveNearestOps {
    perspective: PipelineOp,
    quad: PipelineOp,
    quad_direct: PipelineOp,
    quad_constant: PipelineOp,
    quad_constant_fill: PipelineOp,
    mesh: PipelineOp,
    mesh_constant: PipelineOp,
    mesh_constant_fill: PipelineOp,
}

fn projective_nearest_images() -> (DynamicImage, DynamicImage, DynamicImage) {
    let image = DynamicImage::ImageLuma8(
        GrayImage::from_raw(16, 16, vec![0; 16 * 16]).expect("indexed image"),
    );
    let rgb = DynamicImage::ImageRgb8(
        RgbImage::from_raw(16, 16, vec![0; 16 * 16 * 3]).expect("RGB image"),
    );
    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(16, 16, vec![0; 16 * 16 * 4]).expect("RGBA image"),
    );
    (image, rgb, rgba)
}

fn projective_nearest_ops() -> ProjectiveNearestOps {
    ProjectiveNearestOps {
        perspective: projective_nearest_op(
            TransformMethod::Perspective,
            vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
            Some(7),
        ),
        quad: projective_nearest_op(
            TransformMethod::Quad,
            vec![0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0],
            Some(7),
        ),
        quad_direct: projective_nearest_op(
            TransformMethod::Quad,
            vec![0.0, 0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0],
            Some(7),
        ),
        quad_constant: projective_nearest_op(
            TransformMethod::Quad,
            vec![3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
            None,
        ),
        quad_constant_fill: projective_nearest_op(
            TransformMethod::Quad,
            vec![-1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0],
            None,
        ),
        mesh: projective_nearest_op(
            TransformMethod::Mesh,
            vec![0.0, 0.0, 8.0, 8.0, 0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0],
            Some(7),
        ),
        mesh_constant: projective_nearest_op(
            TransformMethod::Mesh,
            vec![0.0, 0.0, 8.0, 8.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
            None,
        ),
        mesh_constant_fill: projective_nearest_op(
            TransformMethod::Mesh,
            vec![
                0.0, 0.0, 8.0, 8.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0, -1.0, 5.0,
            ],
            None,
        ),
    }
}

fn projective_nearest_op(
    method: TransformMethod,
    data: Vec<f64>,
    palette_fill: Option<u8>,
) -> PipelineOp {
    PipelineOp::Transform {
        w: 8,
        h: 8,
        method,
        data: Arc::from(data),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill,
    }
}

#[test]
fn projective_nearest_proof_is_bounded() {
    let (image, rgb, rgba) = projective_nearest_images();
    let cases = projective_nearest_ops();
    assert_projective_nearest_identity_and_constant_proofs(&image, &rgb, &rgba, &cases);
    assert_projective_nearest_perspective_relocations(&rgb, &rgba);
    assert_projective_nearest_mesh_relocations(&rgb);
    assert_projective_nearest_mesh_rejections(&rgb);
    assert_projective_nearest_overflowing_geometry(&rgb);
    assert_projective_nearest_fractional_and_scaled_maps(&image, &rgb);
    assert_projective_nearest_overflowing_perspective(&rgb);
}

fn assert_projective_nearest_identity_and_constant_proofs(
    image: &DynamicImage,
    rgb: &DynamicImage,
    rgba: &DynamicImage,
    cases: &ProjectiveNearestOps,
) {
    let ProjectiveNearestOps {
        perspective,
        quad,
        quad_direct,
        quad_constant,
        quad_constant_fill,
        mesh,
        mesh_constant,
        mesh_constant_fill,
    } = cases;
    for op in [perspective, quad, quad_direct, mesh] {
        assert!(gpu_projective_nearest_is_exact(
            op,
            image,
            Some("P"),
            (16, 16)
        ));
    }
    for (mode, image) in [("RGB", rgb), ("RGBA", rgba)] {
        for op in [perspective, quad, quad_direct, mesh] {
            assert!(
                gpu_projective_nearest_is_exact(op, image, Some(mode), (16, 16)),
                "identity projective proof for {mode}"
            );
        }
    }
    for (name, op) in [
        ("constant Quad", quad_constant),
        ("constant Quad fill", quad_constant_fill),
        ("constant Mesh", mesh_constant),
        ("constant Mesh fill", mesh_constant_fill),
    ] {
        assert!(
            gpu_projective_nearest_is_exact(op, rgb, Some("RGB"), (16, 16)),
            "{name} proof"
        );
        assert!(
            gpu_projective_nearest_is_exact(op, rgba, Some("RGBA"), (16, 16)),
            "{name} proof for RGBA"
        );
    }
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        &[3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
        ResampleFilter::Bilinear,
        Some("RGB"),
        (8, 8),
    ));
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Mesh,
        &[0.0, 0.0, 8.0, 8.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0],
        ResampleFilter::Bicubic,
        Some("RGBA"),
        (8, 8),
    ));
}

fn assert_projective_nearest_perspective_relocations(rgb: &DynamicImage, rgba: &DynamicImage) {
    let perspective_translate = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Perspective,
        data: Arc::from(vec![1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    let perspective_axis_swap = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Perspective,
        data: Arc::from(vec![0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    for (name, op) in [
        ("integer translation", &perspective_translate),
        ("integer axis swap", &perspective_axis_swap),
    ] {
        for (mode, image) in [("RGB", rgb), ("RGBA", rgba)] {
            assert!(
                gpu_projective_nearest_is_exact(op, image, Some(mode), (16, 16)),
                "{name} proof for {mode}"
            );
        }
    }
    let perspective_translate_negative = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Perspective,
        data: Arc::from(vec![1.0, 0.0, -1.0, 0.0, 1.0, -1.0, 0.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(gpu_projective_nearest_is_exact(
        &perspective_translate_negative,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
    let perspective_signed_unit = [
        [-1.0, 0.0, 8.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0, -1.0, 8.0, 0.0, 0.0],
        [-1.0, 0.0, 8.0, 0.0, -1.0, 8.0, 0.0, 0.0],
        [0.0, -1.0, 8.0, 1.0, 0.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, -1.0, 0.0, 8.0, 0.0, 0.0],
        [0.0, -1.0, 8.0, -1.0, 0.0, 8.0, 0.0, 0.0],
        [-1.0, 0.0, 7.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0, -1.0, 7.0, 0.0, 0.0],
    ];
    for matrix in perspective_signed_unit {
        let op = PipelineOp::Transform {
            w: 8,
            h: 8,
            method: TransformMethod::Perspective,
            data: Arc::from(matrix.to_vec()),
            filter: ResampleFilter::Nearest,
            fill: Some((7, 0, 0, 255)),
            fill_is_none: false,
            palette_fill: None,
        };
        assert!(gpu_projective_nearest_is_exact(
            &op,
            rgb,
            Some("RGB"),
            (16, 16),
        ));
    }
}

fn assert_projective_nearest_mesh_relocations(rgb: &DynamicImage) {
    let mesh_translate = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Mesh,
        data: Arc::from(vec![
            0.0, 0.0, 8.0, 8.0, 1.0, 1.0, 1.0, 9.0, 9.0, 9.0, 9.0, 1.0,
        ]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    let mesh_axis_swap = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Mesh,
        data: Arc::from(vec![
            0.0, 0.0, 8.0, 8.0, 0.0, 0.0, 8.0, 0.0, 8.0, 8.0, 0.0, 8.0,
        ]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    for (name, op) in [
        ("unit mesh translation", &mesh_translate),
        ("unit mesh axis swap", &mesh_axis_swap),
    ] {
        assert!(
            gpu_projective_nearest_is_exact(op, rgb, Some("RGB"), (16, 16)),
            "{name} proof"
        );
    }
}

fn assert_projective_nearest_mesh_rejections(rgb: &DynamicImage) {
    let mesh_scaled = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Mesh,
        data: Arc::from(vec![
            0.0, 0.0, 8.0, 8.0, 0.0, 0.0, 0.0, 16.0, 16.0, 16.0, 16.0, 0.0,
        ]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(!gpu_projective_nearest_is_exact(
        &mesh_scaled,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
    let mesh_extra_record = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Mesh,
        data: Arc::from(vec![
            0.0, 0.0, 8.0, 8.0, 1.0, 1.0, 1.0, 9.0, 9.0, 9.0, 9.0, 1.0, 0.0, 0.0, 8.0, 8.0, 0.0,
            0.0, 0.0, 8.0, 8.0, 8.0, 8.0, 0.0,
        ]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(!gpu_projective_nearest_is_exact(
        &mesh_extra_record,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
    let mesh_constant_extra_record = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Mesh,
        data: Arc::from(vec![
            0.0, 0.0, 8.0, 8.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 0.0, 0.0, 8.0, 8.0, 3.0,
            4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0,
        ]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(!gpu_projective_nearest_is_exact(
        &mesh_constant_extra_record,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
    let scaled_quad_axis = PipelineOp::Transform {
        w: 9,
        h: 6,
        method: TransformMethod::Quad,
        data: Arc::from(vec![0.0, 0.0, 9.0, 0.0, 9.0, 6.0, 0.0, 6.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(gpu_projective_nearest_is_exact(
        &scaled_quad_axis,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
}

fn assert_projective_nearest_overflowing_geometry(rgb: &DynamicImage) {
    let f32_max = f64::from(f32::MAX);
    let overflowing_quad = PipelineOp::Transform {
        w: 3,
        h: 2,
        method: TransformMethod::Quad,
        data: Arc::from(vec![
            f32_max, 0.0, f32_max, 2.0, f32_max, 2.0, -f32_max, 0.0,
        ]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    // The f64 map is wholly outside the source, but the generic f32
    // Quad expression overflows and can produce NaN.  Keep this case on
    // exact host semantic control rather than trusting equal fill
    // classifications from a non-finite source-selection result.
    assert!(!gpu_projective_nearest_is_exact(
        &overflowing_quad,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
    let overflowing_mesh = PipelineOp::Transform {
        w: 3,
        h: 2,
        method: TransformMethod::Mesh,
        data: Arc::from(vec![
            0.0, 0.0, 3.0, 2.0, f32_max, 0.0, -f32_max, 0.0, -f32_max, 2.0, f32_max, 2.0,
        ]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(!gpu_projective_nearest_is_exact(
        &overflowing_mesh,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
}

fn assert_projective_nearest_fractional_and_scaled_maps(image: &DynamicImage, rgb: &DynamicImage) {
    let fractional = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Perspective,
        data: Arc::from(vec![1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: Some(7),
    };
    assert!(gpu_projective_nearest_is_exact(
        &fractional,
        image,
        Some("P"),
        (16, 16)
    ));
    assert!(gpu_projective_nearest_is_exact(
        &fractional,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
    let scaled = PipelineOp::Transform {
        w: 8,
        h: 8,
        method: TransformMethod::Perspective,
        data: Arc::from(vec![2.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(gpu_projective_nearest_is_exact(
        &scaled,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
}

fn assert_projective_nearest_overflowing_perspective(rgb: &DynamicImage) {
    let overflowing = PipelineOp::Transform {
        w: 3,
        h: 1,
        method: TransformMethod::Perspective,
        data: Arc::from(vec![
            f64::from(f32::MAX),
            0.0,
            2.0,
            0.0,
            1.0,
            0.0,
            f64::from(f32::MAX),
            0.0,
        ]),
        filter: ResampleFilter::Nearest,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    // The raw WGSL path overflows at destination x=2 and produces a
    // NaN coordinate; keep this arithmetic-changing boundary on host
    // control even though the ordinary source-selection proof sees fill
    // on both sides of that pixel.
    assert!(!gpu_projective_nearest_is_exact(
        &overflowing,
        rgb,
        Some("RGB"),
        (16, 16)
    ));
}
