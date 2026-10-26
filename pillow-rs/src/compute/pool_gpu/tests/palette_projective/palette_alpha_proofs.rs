use super::{
    gpu_palette_alpha_projective_relocation_is_admitted,
    gpu_projective_filtered_relocation_is_admitted, gpu_projective_nearest_is_exact,
    gpu_transform_should_premultiply,
};
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, GrayImage};
use std::sync::Arc;

#[test]
fn palette_alpha_projective_relocation_proof_is_narrow() {
    assert!(gpu_transform_should_premultiply(
        1,
        None,
        ResampleFilter::Bilinear,
        TransformMethod::Quad,
    ));
    let image = DynamicImage::ImageLuma8(
        GrayImage::from_raw(16, 16, vec![0; 16 * 16]).expect("luma image"),
    );
    let direct = [1.0, 0.0, -2.0, 0.0, 1.0, 1.0, 0.0, 0.0];
    let swapped = [0.0, 1.0, 3.0, 1.0, 0.0, -4.0, 0.0, 0.0];
    let quad_axis_swap = [0.0, 0.0, 6.0, 0.0, 6.0, 9.0, 0.0, 9.0];
    let quad_direct = [0.0, 0.0, 0.0, 6.0, 9.0, 6.0, 9.0, 0.0];
    let quad_constant_fractional = [3.25, 4.0, 3.25, 4.0, 3.25, 4.0, 3.25, 4.0];
    let mesh_relocation = [0.0, 0.0, 9.0, 6.0, 1.0, 2.0, 1.0, 8.0, 10.0, 8.0, 10.0, 2.0];
    assert_palette_alpha_nearest_proofs(
        &direct,
        &swapped,
        &quad_axis_swap,
        &quad_direct,
        &quad_constant_fractional,
    );
    assert_palette_alpha_filtered_shape_proofs(&image, &quad_constant_fractional, &mesh_relocation);
    assert_palette_alpha_filtered_mode_proofs(
        &direct,
        &swapped,
        &quad_axis_swap,
        &quad_direct,
        &mesh_relocation,
    );
}

fn assert_palette_alpha_nearest_proofs(
    direct: &[f64; 8],
    swapped: &[f64; 8],
    quad_axis_swap: &[f64; 8],
    quad_direct: &[f64; 8],
    quad_constant_fractional: &[f64; 8],
) {
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        direct,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        swapped,
        ResampleFilter::Nearest,
        (5, 7),
    ));
    let reflected = [-1.0, 0.0, 9.0, 0.0, 1.0, 6.0, 0.0, 0.0];
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        &reflected,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        direct,
        ResampleFilter::Bilinear,
        (9, 6),
    ));
    assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        direct,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        quad_axis_swap,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        quad_direct,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    let quad_constant = [3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0];
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        &quad_constant,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Quad,
        quad_constant_fractional,
        ResampleFilter::Nearest,
        (9, 6),
    ));
}

const PALETTE_ALPHA_MESH_CONSTANT: [f64; 12] =
    [0.0, 0.0, 9.0, 6.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0];

fn assert_palette_alpha_filtered_shape_proofs(
    image: &DynamicImage,
    quad_constant_fractional: &[f64; 8],
    mesh_relocation: &[f64; 12],
) {
    assert_palette_alpha_quad_filtered_shape_proofs(quad_constant_fractional);
    assert_palette_alpha_mesh_nearest_shape_proofs(image, mesh_relocation);
    assert_palette_alpha_mesh_filtered_shape_proofs();
}

fn assert_palette_alpha_quad_filtered_shape_proofs(quad_constant_fractional: &[f64; 8]) {
    let quad_constant_half = [1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5];
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        &quad_constant_half,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        quad_constant_fractional,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
}

fn assert_palette_alpha_mesh_nearest_shape_proofs(
    image: &DynamicImage,
    mesh_relocation: &[f64; 12],
) {
    let mesh_filtered = PipelineOp::Transform {
        w: 9,
        h: 6,
        method: TransformMethod::Mesh,
        data: Arc::from(mesh_relocation.to_vec()),
        filter: ResampleFilter::Bilinear,
        fill: Some((7, 0, 0, 255)),
        fill_is_none: false,
        palette_fill: None,
    };
    assert!(gpu_projective_nearest_is_exact(
        &mesh_filtered,
        image,
        Some("L"),
        (16, 16)
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Mesh,
        mesh_relocation,
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Mesh,
        &PALETTE_ALPHA_MESH_CONSTANT,
        ResampleFilter::Nearest,
        (9, 6),
    ));
}

fn assert_palette_alpha_mesh_filtered_shape_proofs() {
    let mesh_constant_half = [0.0, 0.0, 9.0, 6.0, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5, 1.5, 4.5];
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Mesh,
        &mesh_constant_half,
        ResampleFilter::Bicubic,
        Some("PA"),
        (9, 6),
    ));
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Mesh,
        &PALETTE_ALPHA_MESH_CONSTANT,
        ResampleFilter::Bicubic,
        Some("PA"),
        (9, 6),
    ));
    let mesh_constant_extra = [
        0.0, 0.0, 9.0, 6.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 3.0, 4.0, 0.0, 0.0,
    ];
    assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Mesh,
        &mesh_constant_extra,
        ResampleFilter::Nearest,
        (9, 6),
    ));
}

fn assert_palette_alpha_filtered_mode_proofs(
    direct: &[f64; 8],
    swapped: &[f64; 8],
    quad_axis_swap: &[f64; 8],
    quad_direct: &[f64; 8],
    mesh_relocation: &[f64; 12],
) {
    assert_palette_alpha_perspective_filtered_proofs(direct, swapped);
    assert_palette_alpha_quad_mesh_filtered_proofs(
        direct,
        quad_axis_swap,
        quad_direct,
        mesh_relocation,
    );
}

fn assert_palette_alpha_perspective_filtered_proofs(direct: &[f64; 8], swapped: &[f64; 8]) {
    assert_palette_alpha_perspective_primary_proofs(direct, swapped);
    assert_palette_alpha_perspective_constant_proofs();
    assert_palette_alpha_perspective_mode_proofs(direct, swapped);
}

fn assert_palette_alpha_perspective_primary_proofs(direct: &[f64; 8], swapped: &[f64; 8]) {
    assert!(!gpu_palette_alpha_projective_relocation_is_admitted(
        TransformMethod::Perspective,
        &[1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0],
        ResampleFilter::Nearest,
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        direct,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        swapped,
        ResampleFilter::Bicubic,
        Some("PA"),
        (5, 7),
    ));
}

fn assert_palette_alpha_perspective_constant_proofs() {
    let perspective_constant_half = [0.0, 0.0, 1.5, 0.0, 0.0, 4.5, 0.0, 0.0];
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        &perspective_constant_half,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    let perspective_constant_fractional = [0.0, 0.0, 1.25, 0.0, 0.0, 4.5, 0.0, 0.0];
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        &perspective_constant_fractional,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
}

fn assert_palette_alpha_perspective_mode_proofs(direct: &[f64; 8], swapped: &[f64; 8]) {
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        direct,
        ResampleFilter::Bilinear,
        Some("LA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        swapped,
        ResampleFilter::Bicubic,
        Some("RGBA"),
        (5, 7),
    ));
    let fractional = [1.0, 0.0, 0.25, 0.0, 1.0, 0.0, 0.0, 0.0];
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        &fractional,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Perspective,
        &fractional,
        ResampleFilter::Bilinear,
        Some("LA"),
        (9, 6),
    ));
}

fn assert_palette_alpha_quad_mesh_filtered_proofs(
    direct: &[f64; 8],
    quad_axis_swap: &[f64; 8],
    quad_direct: &[f64; 8],
    mesh_relocation: &[f64; 12],
) {
    assert!(!gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        direct,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        quad_direct,
        ResampleFilter::Bilinear,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Quad,
        quad_axis_swap,
        ResampleFilter::Bicubic,
        Some("PA"),
        (9, 6),
    ));
    assert!(gpu_projective_filtered_relocation_is_admitted(
        TransformMethod::Mesh,
        mesh_relocation,
        ResampleFilter::Bicubic,
        Some("PA"),
        (9, 6),
    ));
}
