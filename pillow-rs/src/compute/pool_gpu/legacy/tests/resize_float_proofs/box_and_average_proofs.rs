use super::{
    gpu_f_resize_box_average_is_exact, gpu_f_resize_box_copy_is_exact, gpu_f_resize_constant_bits,
    gpu_resize_coefficients, gpu_resize_nearest_uses_coefficients,
};
use crate::pipeline::{PipelineOp, PixelMode, ResampleFilter};
use crate::raster::{DynamicImage, RgbaImage};

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
    assert_f_resize_box_copy_positive_chains(&image);
    assert_f_resize_box_copy_rejected_chains(&image);
    assert_f_resize_box_copy_special_values();
}

fn f_resize_box_operation(w: u32, h: u32) -> PipelineOp {
    PipelineOp::Resize {
        w,
        h,
        filter: ResampleFilter::Box,
    }
}

fn assert_f_resize_box_copy_positive_chains(image: &DynamicImage) {
    assert!(gpu_f_resize_box_copy_is_exact(
        &[f_resize_box_operation(6, 4)],
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_box_copy_is_exact(
        &[f_resize_box_operation(3, 2), f_resize_box_operation(12, 8)],
        image,
        Some("F")
    ));
    assert!(gpu_f_resize_box_copy_is_exact(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::from(vec![0u8; 24]),
                mode: PixelMode::F,
            },
            f_resize_box_operation(6, 4),
        ],
        image,
        Some("F")
    ));
}

fn assert_f_resize_box_copy_rejected_chains(image: &DynamicImage) {
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::from(vec![0u8; 4]),
                mode: PixelMode::F,
            },
            f_resize_box_operation(6, 4),
        ],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[f_resize_box_operation(2, 2)],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[PipelineOp::Resize {
            w: 6,
            h: 4,
            filter: ResampleFilter::Bilinear,
        }],
        image,
        Some("F")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[f_resize_box_operation(6, 4)],
        image,
        Some("I")
    ));
    assert!(!gpu_f_resize_box_copy_is_exact(
        &[f_resize_box_operation(6, 4), PipelineOp::Mirror],
        image,
        Some("F")
    ));
}

fn assert_f_resize_box_copy_special_values() {
    let negative_zero = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, (-0.0f32).to_le_bytes().repeat(6)).unwrap(),
    );
    assert!(gpu_f_resize_box_copy_is_exact(
        &[f_resize_box_operation(6, 4)],
        &negative_zero,
        Some("F")
    ));
    let nan = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, f32::NAN.to_le_bytes().repeat(6)).unwrap(),
    );
    assert!(gpu_f_resize_box_copy_is_exact(
        &[f_resize_box_operation(6, 4)],
        &nan,
        Some("F")
    ));
    let infinity = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(3, 2, f32::INFINITY.to_le_bytes().repeat(6)).unwrap(),
    );
    assert!(gpu_f_resize_box_copy_is_exact(
        &[f_resize_box_operation(6, 4)],
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
