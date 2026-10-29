use crate::pipeline::{ColorMode, PipelineOp, ResampleFilter, TransformMethod};
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};
use std::sync::Arc;

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_expand_requires_matching_singleton_layout() {
    let expand = PipelineOp::Expand {
        border: 1,
        fill: (7, 11, 13, 17),
    };
    assert_gpu_native_expand_matching_layouts(&expand);
    assert_gpu_native_expand_rejects_mismatches(&expand);
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_expand_matching_layouts(expand: &PipelineOp) {
    let cases = [
        (
            DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![3, 5]).unwrap()),
            None,
            Some(1),
        ),
        (
            DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(2, 1, vec![3, 19, 5, 23]).unwrap()),
            None,
            Some(2),
        ),
        (
            DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17]).unwrap()),
            Some("RGB"),
            Some(3),
        ),
        (
            DynamicImage::ImageRgb8(RgbImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17]).unwrap()),
            Some("HSV"),
            Some(3),
        ),
        (
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17, 19, 23]).unwrap(),
            ),
            None,
            Some(4),
        ),
        (
            DynamicImage::ImageRgba8(
                RgbaImage::from_raw(2, 1, vec![3, 5, 7, 11, 13, 17, 19, 23]).unwrap(),
            ),
            Some("CMYK"),
            Some(4),
        ),
    ];
    for (image, mode, expected) in cases {
        assert_eq!(
            super::gpu_native_expand_channels(std::slice::from_ref(expand), &image, mode),
            expected,
            "mode {mode:?}"
        );
    }
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_expand_rejects_mismatches(expand: &PipelineOp) {
    let rgb = DynamicImage::ImageRgb8(RgbImage::from_raw(1, 1, vec![1, 2, 3]).unwrap());
    assert_eq!(
        super::gpu_native_expand_channels(std::slice::from_ref(expand), &rgb, Some("CMYK")),
        None
    );
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(1, 1, vec![3]).unwrap());
    assert_eq!(
        super::gpu_native_expand_channels(std::slice::from_ref(expand), &luma, Some("1")),
        None
    );
    let multiple = [expand.clone(), PipelineOp::Duplicate];
    assert_eq!(
        super::gpu_native_expand_channels(&multiple, &rgb, Some("RGB")),
        None
    );
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_native_grayscale_inputs_require_singleton_exact_storage() {
    let grayscale = PipelineOp::Grayscale;
    let rgb = DynamicImage::ImageRgb8(
        RgbImage::from_raw(3, 1, vec![12, 34, 56, 78, 90, 123, 234, 210, 98]).unwrap(),
    );
    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            3,
            1,
            vec![12, 34, 56, 78, 90, 123, 234, 210, 98, 76, 54, 32],
        )
        .unwrap(),
    );
    assert_gpu_native_grayscale_rgb_input_modes(&grayscale, &rgb);
    assert_gpu_native_grayscale_rgba_input_modes(&grayscale, &rgba);
    assert_gpu_native_grayscale_rejected_inputs(&grayscale, &rgb, &rgba);
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_grayscale_rgb_input_modes(grayscale: &PipelineOp, rgb: &DynamicImage) {
    assert!(super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgb,
        None
    ));
    assert!(super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgb,
        Some("RGB")
    ));
    assert!(super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgb,
        Some("YCbCr")
    ));
    assert!(!super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgb,
        Some("HSV")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_grayscale_rgba_input_modes(grayscale: &PipelineOp, rgba: &DynamicImage) {
    assert!(!super::gpu_native_grayscale_rgb_input(
        std::slice::from_ref(grayscale),
        rgba,
        Some("RGB")
    ));
    assert!(super::gpu_native_grayscale_cmyk_input(
        std::slice::from_ref(grayscale),
        rgba,
        Some("CMYK")
    ));
    assert!(!super::gpu_native_grayscale_cmyk_input(
        std::slice::from_ref(grayscale),
        rgba,
        None
    ));
    assert!(!super::gpu_native_grayscale_cmyk_input(
        std::slice::from_ref(grayscale),
        rgba,
        Some("RGB")
    ));
}

#[cfg(target_endian = "little")]
fn assert_gpu_native_grayscale_rejected_inputs(
    grayscale: &PipelineOp,
    rgb: &DynamicImage,
    rgba: &DynamicImage,
) {
    let multiple = [grayscale.clone(), PipelineOp::Duplicate];
    assert!(!super::gpu_native_grayscale_rgb_input(
        &multiple,
        rgb,
        Some("RGB")
    ));
    assert!(!super::gpu_native_grayscale_cmyk_input(
        &multiple,
        rgba,
        Some("CMYK")
    ));
    let empty = DynamicImage::ImageRgb8(RgbImage::new(0, 1));
    assert!(!super::gpu_native_grayscale_rgb_input(
        &[PipelineOp::Grayscale],
        &empty,
        Some("RGB")
    ));
    let empty_cmyk = DynamicImage::ImageRgba8(RgbaImage::new(0, 1));
    assert!(!super::gpu_native_grayscale_cmyk_input(
        &[PipelineOp::Grayscale],
        &empty_cmyk,
        Some("CMYK")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_convert_requires_nonempty_native_l_to_rgba() {
    let convert_rgba = PipelineOp::Convert {
        mode: ColorMode::RGBA,
        matrix: None,
        dither: None,
    };
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![31, 207]).unwrap());
    assert!(super::gpu_packed_luma_convert_input(
        std::slice::from_ref(&convert_rgba),
        &luma,
        None
    ));
    assert!(super::gpu_packed_luma_convert_input(
        std::slice::from_ref(&convert_rgba),
        &luma,
        Some("L")
    ));

    let rgb = PipelineOp::Convert {
        mode: ColorMode::RGB,
        matrix: None,
        dither: None,
    };
    assert!(!super::gpu_packed_luma_convert_input(
        std::slice::from_ref(&rgb),
        &luma,
        Some("L")
    ));
    let multiple = [convert_rgba.clone(), PipelineOp::Duplicate];
    assert!(!super::gpu_packed_luma_convert_input(
        &multiple,
        &luma,
        Some("L")
    ));
    let empty = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert!(!super::gpu_packed_luma_convert_input(
        std::slice::from_ref(&convert_rgba),
        &empty,
        Some("L")
    ));
}

#[test]
#[cfg(target_endian = "little")]
fn gpu_packed_luma_transform_admits_only_nonempty_native_l() {
    let transform = PipelineOp::Transform {
        w: 2,
        h: 1,
        method: TransformMethod::Affine,
        data: Arc::from(vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0]),
        filter: ResampleFilter::Nearest,
        fill: None,
        fill_is_none: true,
        palette_fill: None,
    };
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![31, 207]).unwrap());
    assert!(super::gpu_packed_luma_transform_input(
        std::slice::from_ref(&transform),
        &luma,
        None
    ));
    assert!(super::gpu_packed_luma_transform_input(
        std::slice::from_ref(&transform),
        &luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_transform_input(
        std::slice::from_ref(&transform),
        &luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_luma_transform_input(
        &[transform.clone(), PipelineOp::Duplicate],
        &luma,
        Some("L")
    ));
    let empty = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert!(!super::gpu_packed_luma_transform_input(
        std::slice::from_ref(&transform),
        &empty,
        Some("L")
    ));
}

#[test]
fn gpu_packed_luma_point_admits_lut_and_lowered_l_autocontrast() {
    let eval = PipelineOp::Eval {
        lut: vec![0; 256].into(),
    };
    let equalize = PipelineOp::Equalize;
    let luma = DynamicImage::ImageLuma8(GrayImage::from_raw(2, 1, vec![31, 207]).unwrap());
    assert_gpu_packed_luma_point_admissions(&eval, &equalize, &luma);
    assert_gpu_packed_luma_point_rejections(&eval, &equalize, &luma);
}

fn assert_gpu_packed_luma_point_admissions(
    eval: &PipelineOp,
    equalize: &PipelineOp,
    luma: &DynamicImage,
) {
    assert!(super::gpu_packed_luma_point_input(
        true,
        false,
        std::slice::from_ref(eval),
        luma,
        None
    ));
    assert!(super::gpu_packed_luma_point_input(
        false,
        true,
        std::slice::from_ref(eval),
        luma,
        Some("L")
    ));
    assert!(super::gpu_packed_luma_point_input(
        false,
        false,
        std::slice::from_ref(equalize),
        luma,
        Some("L")
    ));
}

fn assert_gpu_packed_luma_point_rejections(
    eval: &PipelineOp,
    equalize: &PipelineOp,
    luma: &DynamicImage,
) {
    assert!(!super::gpu_packed_luma_point_input(
        false,
        false,
        std::slice::from_ref(equalize),
        luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_luma_point_input(
        false,
        false,
        std::slice::from_ref(eval),
        luma,
        Some("L")
    ));
    assert!(!super::gpu_packed_luma_point_input(
        false,
        true,
        std::slice::from_ref(eval),
        luma,
        Some("P")
    ));
    assert!(!super::gpu_packed_luma_point_input(
        false,
        true,
        &[eval.clone(), PipelineOp::Duplicate],
        luma,
        Some("L")
    ));
    let empty = DynamicImage::ImageLuma8(GrayImage::new(0, 1));
    assert!(!super::gpu_packed_luma_point_input(
        false,
        true,
        std::slice::from_ref(eval),
        &empty,
        Some("L")
    ));
}
