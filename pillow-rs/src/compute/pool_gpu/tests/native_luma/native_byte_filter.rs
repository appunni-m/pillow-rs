use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage};

#[cfg(target_endian = "little")]
struct NativeByteFilterCases {
    blur: PipelineOp,
    box_blur: PipelineOp,
    median: PipelineOp,
    max_filter: PipelineOp,
    min_filter: PipelineOp,
    rank_filter: PipelineOp,
    binomial3x3_kernel: [f32; 9],
    filter_3x3: PipelineOp,
    filter_5x5: PipelineOp,
    luma: DynamicImage,
    aligned_luma: DynamicImage,
    la: DynamicImage,
}

#[cfg(target_endian = "little")]
fn native_byte_filter_cases() -> NativeByteFilterCases {
    let blur = PipelineOp::GaussianBlur { sigma: 2.0 };
    let box_blur = PipelineOp::BoxBlur { radius: 1 };
    let median = PipelineOp::MedianFilter { size: 3 };
    let max_filter = PipelineOp::MaxFilter { size: 3 };
    let min_filter = PipelineOp::MinFilter { size: 3 };
    let rank_filter = PipelineOp::RankFilter { size: 3, rank: 1 };
    let binomial3x3_kernel = [1.0, 2.0, 1.0, 2.0, 4.0, 2.0, 1.0, 2.0, 1.0];
    let filter_3x3 = PipelineOp::Filter3x3 {
        kernel: binomial3x3_kernel,
        scale: 16.0,
        offset: 0.0,
    };
    let weights = [1.0, 4.0, 6.0, 4.0, 1.0];
    let kernel = std::array::from_fn(|index| weights[index / 5] * weights[index % 5]);
    let filter_5x5 = PipelineOp::Filter5x5 {
        kernel,
        scale: 256.0,
        offset: 0.0,
    };
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(65, 47, vec![37; 65 * 47]).unwrap());
    let aligned_luma =
        DynamicImage::ImageLuma8(GrayImage::from_raw(8, 6, vec![37; 8 * 6]).unwrap());
    let la =
        DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(65, 47, vec![37; 65 * 47 * 2]).unwrap());
    NativeByteFilterCases {
        blur,
        box_blur,
        median,
        max_filter,
        min_filter,
        rank_filter,
        binomial3x3_kernel,
        filter_3x3,
        filter_5x5,
        luma,
        aligned_luma,
        la,
    }
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_native_byte_filter_admits_exact_native_l_and_la_singletons() {
    let cases = native_byte_filter_cases();
    assert_luma_native_byte_filter_admission(&cases);
    assert_la_native_byte_filter_admission(&cases);
    assert_la_native_byte_filter_rejections(&cases);
    assert_native_byte_filter_mode_and_shape_rejections(&cases);
}

#[cfg(target_endian = "little")]
fn assert_luma_native_byte_filter_admission(cases: &NativeByteFilterCases) {
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.luma,
        None
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.luma,
        Some("L")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::GaussianBlur { sigma: -2.0 }],
        &cases.luma,
        Some("L")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::GaussianBlur { sigma: 0.0 }],
        &cases.luma,
        Some("L")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.box_blur),
        &cases.luma,
        None
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.box_blur),
        &cases.luma,
        Some("L")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.aligned_luma,
        None
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.aligned_luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.aligned_luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::Filter5x5 {
            kernel: [1.0; 25],
            scale: 0.0,
            offset: 0.0,
        }],
        &cases.aligned_luma,
        Some("L")
    ));
}

#[cfg(target_endian = "little")]
fn assert_la_native_byte_filter_admission(cases: &NativeByteFilterCases) {
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_3x3),
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_3x3),
        &cases.la,
        None
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::Filter3x3 {
            kernel: [1.0; 9],
            scale: 16.0,
            offset: 0.0,
        }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::Filter3x3 {
            kernel: cases.binomial3x3_kernel,
            scale: 16.0,
            offset: 1.0,
        }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.filter_5x5),
        &cases.la,
        None
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::Filter5x5 {
            kernel: [1.0; 25],
            scale: 0.0,
            offset: 0.0,
        }],
        &cases.la,
        Some("LA")
    ));
}

#[cfg(target_endian = "little")]
fn assert_la_native_byte_filter_rejections(cases: &NativeByteFilterCases) {
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.box_blur),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.median),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.max_filter),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.min_filter),
        &cases.la,
        Some("LA")
    ));
    assert!(super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.rank_filter),
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.median),
        &cases.la,
        None
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::MedianFilter { size: 5 }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::MaxFilter { size: 5 }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::MinFilter { size: 5 }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::RankFilter { size: 5, rank: 1 }],
        &cases.la,
        Some("LA")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::RankFilter { size: 3, rank: 9 }],
        &cases.la,
        Some("LA")
    ));
}

#[cfg(target_endian = "little")]
fn assert_native_byte_filter_mode_and_shape_rejections(cases: &NativeByteFilterCases) {
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.median),
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.la,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::BoxBlur { radius: 0 }],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::BoxBlur { radius: 2 }],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::BoxBlurXY {
            radius_x: 1.0,
            radius_y: 1.0,
            passes: 1,
        }],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[cases.blur.clone(), PipelineOp::Duplicate],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[cases.box_blur.clone(), PipelineOp::Duplicate],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &cases.luma,
        Some("RGB")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &DynamicImage::ImageRgb8(RgbImage::from_raw(65, 47, vec![0; 65 * 47 * 3]).unwrap()),
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        &[PipelineOp::GaussianBlur { sigma: f32::NAN }],
        &cases.luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_native_byte_filter_input(
        std::slice::from_ref(&cases.blur),
        &DynamicImage::ImageLuma8(GrayImage::new(0, 1)),
        Some("L")
    ));
}
