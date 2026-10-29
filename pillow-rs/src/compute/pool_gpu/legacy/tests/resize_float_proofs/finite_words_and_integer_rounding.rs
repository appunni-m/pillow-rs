use super::{gpu_f_resize_dyadic_is_exact, gpu_f64_integer_to_f32, gpu_float_filter_is_supported};
use crate::pipeline::{PipelineOp, PixelMode, ResampleFilter};
use crate::raster::{DynamicImage, RgbaImage};

fn f32_putdata_bytes(values: [f32; 4]) -> bytes::Bytes {
    bytes::Bytes::from(
        values
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect::<Vec<_>>(),
    )
}

#[test]
fn f_order_filter_putdata_requires_finite_words() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            2,
            2,
            [1.0f32, 2.0, 3.0, 4.0]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    let rank = PipelineOp::RankFilter { size: 3, rank: 4 };
    assert!(gpu_float_filter_is_supported(
        &[
            PipelineOp::PutData {
                data: f32_putdata_bytes([4.0, 3.0, 2.0, 1.0]),
                mode: PixelMode::F,
            },
            rank.clone(),
        ],
        &image,
    ));
    for special in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(!gpu_float_filter_is_supported(
            &[
                PipelineOp::PutData {
                    data: f32_putdata_bytes([special, 3.0, 2.0, 1.0]),
                    mode: PixelMode::F,
                },
                rank.clone(),
            ],
            &image,
        ));
    }
    assert!(!gpu_float_filter_is_supported(
        &[
            PipelineOp::PutData {
                data: bytes::Bytes::from(vec![0u8; 4]),
                mode: PixelMode::F,
            },
            rank.clone(),
        ],
        &image,
    ));
    assert!(!gpu_float_filter_is_supported(
        &[
            PipelineOp::PutData {
                data: f32_putdata_bytes([4.0, 3.0, 2.0, 1.0]),
                mode: PixelMode::I,
            },
            rank,
        ],
        &image,
    ));
}

#[test]
fn f_resize_f64_subnormal_rounding_is_ties_to_even() {
    assert_eq!(gpu_f64_integer_to_f32(1, -149), Some(0x0000_0001));
    assert_eq!(gpu_f64_integer_to_f32(1, -150), Some(0));
    assert_eq!(gpu_f64_integer_to_f32(3, -151), Some(0x0000_0001));
    assert_eq!(gpu_f64_integer_to_f32(3, -150), Some(0x0000_0002));
    assert_eq!(
        gpu_f64_integer_to_f32((1i128 << 23) - 1, -149),
        Some(0x007f_ffff)
    );
    assert_eq!(gpu_f64_integer_to_f32(1i128 << 23, -149), Some(0x0080_0000));
    assert_eq!(gpu_f64_integer_to_f32(-1, -149), Some(0x8000_0001));
    assert_eq!(gpu_f64_integer_to_f32(-1, -200), Some(0x8000_0000));
    assert_eq!(gpu_f64_integer_to_f32(1, 128), Some(0x7f80_0000));
    assert_eq!(gpu_f64_integer_to_f32(-1, 128), Some(0xff80_0000));
}

#[test]
fn f_resize_integer_lowering_proves_signed_arbitrary_significands_and_two_axes() {
    let image = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            2,
            2,
            [0.1f32, 0.3, 1.7, 2.9]
                .into_iter()
                .flat_map(f32::to_le_bytes)
                .collect(),
        )
        .unwrap(),
    );
    let one_axis = PipelineOp::Resize {
        w: 1,
        h: 2,
        filter: ResampleFilter::Bilinear,
    };
    assert!(gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(&one_axis),
        &image,
        Some("F")
    ));

    // The horizontal integer proof materializes the exact rounded f32
    // intermediate before proving the vertical pass.
    assert!(gpu_f_resize_dyadic_is_exact(
        &[PipelineOp::Resize {
            w: 1,
            h: 1,
            filter: ResampleFilter::Bilinear,
        }],
        &image,
        Some("F")
    ));

    let mixed_sign = DynamicImage::ImageRgba8(
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
    assert!(gpu_f_resize_dyadic_is_exact(
        std::slice::from_ref(&one_axis),
        &mixed_sign,
        Some("F")
    ));
}
