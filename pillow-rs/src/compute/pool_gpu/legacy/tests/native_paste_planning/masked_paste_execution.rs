use super::{
    gpu_buffer_capacity_exceeds_limits, gpu_dispatch_dimensions_require_cpu,
    gpu_native_masked_byte_paste_layout, gpu_native_masked_byte_paste_shader,
    plan_gpu_native_masked_byte_paste,
};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbaImage};
use crate::{Backend, Image};
use std::sync::Arc;

#[test]
fn compact_masked_la_paste_can_fit_when_generic_grids_and_buffers_do_not() {
    let source = Arc::new(crate::image::Image::new(17, 1, "LA", (0, 0, 0, 255)).unwrap());
    let mask = Arc::new(crate::image::Image::new(17, 1, "L", (127, 0, 0, 0)).unwrap());
    let operation = PipelineOp::Paste {
        source,
        x: 0,
        y: 0,
        w: 17,
        h: 1,
        mask: Some(mask),
        mask_alpha: false,
    };

    assert!(gpu_dispatch_dimensions_require_cpu(
        std::slice::from_ref(&operation),
        (17, 1),
        1,
        Some("LA"),
        false,
    ));
    assert!(gpu_buffer_capacity_exceeds_limits(17, 56, 56));

    let compact = plan_gpu_native_masked_byte_paste(17, 1, 17, 1, 17, 1, 2, 1, 56, 56)
        .expect("compact LA transport fits both adapter bindings and its grid");
    assert_eq!(compact.workgroups, 1);
    assert_eq!(compact.destination_transfer_bytes, 36);
    assert_eq!(compact.input_transfer_bytes, 56);
    assert!(!gpu_buffer_capacity_exceeds_limits(
        compact.word_count,
        56,
        56
    ));
}

#[test]
fn native_masked_pa_paste_uses_a_distinct_logical_mode_shader() {
    let pa = gpu_native_masked_byte_paste_shader("PA", 2)
        .expect("PA selects its native two-byte pipeline");
    let la = gpu_native_masked_byte_paste_shader("LA", 2)
        .expect("LA selects its own native two-byte pipeline");
    assert_eq!(pa.0, "__internal_paste_native_masked_pa");
    assert_eq!(pa.1, "paste_native_masked_pa.wgsl");
    assert!(pa.2.contains("PA index and alpha are blended independently"));
    assert_eq!(la.0, "__internal_paste_native_masked_la");
    assert_ne!(pa.0, la.0);
    assert!(gpu_native_masked_byte_paste_shader("PA", 3).is_none());
}

#[test]
fn native_masked_rgba_paste_selects_its_native_four_byte_shader() {
    let rgba =
        gpu_native_masked_byte_paste_shader("RGBA", 4).expect("RGBA selects its four-byte shader");
    assert_eq!(rgba.0, "__internal_paste_native_masked_rgba");
    assert_eq!(rgba.1, "paste_native_masked_rgba.wgsl");
    assert!(
        rgba.2
            .contains("word_index = gid.x + gid.y * params.workgroups_x")
    );
    assert!(gpu_native_masked_byte_paste_shader("RGBA", 3).is_none());
    assert!(gpu_native_masked_byte_paste_shader("RGBa", 4).is_none());
}

#[test]
fn native_masked_pa_layout_keeps_the_palette_alpha_mode_tag() {
    let bytes = [1, 2, 3, 4, 5, 6];
    let source_image = Arc::new(Image::frombytes("PA", (3, 1), &bytes).unwrap());
    let mask_image = Arc::new(Image::frombytes("L", (3, 1), &[0, 127, 255]).unwrap());
    let destination =
        DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(3, 1, bytes.to_vec()).unwrap());
    let source = DynamicImage::ImageLumaA8(GrayAlphaImage::from_raw(3, 1, bytes.to_vec()).unwrap());
    let mask = DynamicImage::ImageLuma8(GrayImage::from_raw(3, 1, vec![0, 127, 255]).unwrap());
    let op = PipelineOp::Paste {
        source: source_image,
        x: 0,
        y: 0,
        w: 3,
        h: 1,
        mask: Some(mask_image),
        mask_alpha: false,
    };

    assert_eq!(
        gpu_native_masked_byte_paste_layout(&op, &destination, &source, &mask, Some("PA")),
        Some(((0, 0), 2, "PA"))
    );
}

#[test]
fn native_masked_rgba_layout_keeps_four_stored_channels() {
    let source_bytes = [1u8; 5 * 3 * 4];
    let destination_bytes = [2u8; 7 * 4 * 4];
    let mask_bytes = [127u8; 5 * 3];
    let source_image = Arc::new(Image::frombytes("RGBA", (5, 3), &source_bytes).unwrap());
    let mask_image = Arc::new(Image::frombytes("L", (5, 3), &mask_bytes).unwrap());
    let destination =
        DynamicImage::ImageRgba8(RgbaImage::from_raw(7, 4, destination_bytes.to_vec()).unwrap());
    let source =
        DynamicImage::ImageRgba8(RgbaImage::from_raw(5, 3, source_bytes.to_vec()).unwrap());
    let mask = DynamicImage::ImageLuma8(GrayImage::from_raw(5, 3, mask_bytes.to_vec()).unwrap());
    let op = PipelineOp::Paste {
        source: source_image,
        x: -1,
        y: 1,
        w: 5,
        h: 3,
        mask: Some(mask_image),
        mask_alpha: false,
    };

    assert_eq!(
        gpu_native_masked_byte_paste_layout(&op, &destination, &source, &mask, Some("RGBA")),
        Some(((-1, 1), 4, "RGBA"))
    );
}

#[test]
#[cfg(target_endian = "little")]
fn native_masked_rgb_and_hsv_paste_use_the_three_byte_shader() {
    if !native_masked_rgb_gpu_available_or_skip() {
        return;
    }
    let _restore = RestoreGpuDiagnostics::enable();
    let _ = Backend::take_gpu_shader_coverage();
    let (destination_bytes, source_bytes, mask_bytes) = native_masked_rgb_hsv_fixture_bytes();
    for mode in ["RGB", "HSV"] {
        for position in [(-1, -1), (2, 1)] {
            assert_native_masked_rgb_hsv_case(
                mode,
                position,
                &destination_bytes,
                &source_bytes,
                &mask_bytes,
            );
        }
    }
}

#[cfg(target_endian = "little")]
struct RestoreGpuDiagnostics {
    telemetry: bool,
    shader_coverage: bool,
}

#[cfg(target_endian = "little")]
impl RestoreGpuDiagnostics {
    fn enable() -> Self {
        Self {
            telemetry: Backend::set_pipeline_telemetry_enabled(true),
            shader_coverage: Backend::set_gpu_shader_coverage_enabled(true),
        }
    }
}

#[cfg(target_endian = "little")]
impl Drop for RestoreGpuDiagnostics {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.telemetry);
        Backend::set_gpu_shader_coverage_enabled(self.shader_coverage);
    }
}

#[cfg(target_endian = "little")]
fn native_masked_rgb_gpu_available_or_skip() -> bool {
    match super::GpuPool::ensure_init() {
        Ok(_) => true,
        Err(error)
            if error.to_string().contains("GPU adapter not available")
                || error
                    .to_string()
                    .contains("GPU device initialization failed") =>
        {
            false
        }
        Err(error) => panic!("native masked RGB GPU initialization failed: {error}"),
    }
}

#[cfg(target_endian = "little")]
fn native_masked_rgb_hsv_fixture_bytes() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let mut destination_bytes = (0..45)
        .map(|index: i32| (index.wrapping_mul(41).wrapping_add(19)) as u8)
        .collect::<Vec<_>>();
    let mut source_bytes = (0..45)
        .map(|index: i32| 231i32.wrapping_sub(index.wrapping_mul(17)) as u8)
        .collect::<Vec<_>>();
    for pixel in [0usize, 1, 7, 8] {
        let start = pixel.saturating_mul(3);
        let end = start.saturating_add(3);
        destination_bytes[start..end].copy_from_slice(&[100, 100, 100]);
    }
    for pixel in [0usize, 1, 6, 7] {
        let start = pixel.saturating_mul(3);
        let end = start.saturating_add(3);
        source_bytes[start..end].copy_from_slice(&[101, 99, 100]);
    }
    let mut mask_bytes = vec![
        0, 1, 127, 128, 254, 255, 127, 128, 127, 128, 254, 255, 254, 1, 127,
    ];
    mask_bytes[0..2].copy_from_slice(&[127, 128]);
    (destination_bytes, source_bytes, mask_bytes)
}

#[cfg(target_endian = "little")]
fn assert_native_masked_rgb_hsv_case(
    mode: &str,
    position: (i32, i32),
    destination_bytes: &[u8],
    source_bytes: &[u8],
    mask_bytes: &[u8],
) {
    use crate::ops::paste::PasteSource;

    let source = Image::frombytes(mode, (5, 3), source_bytes).unwrap();
    let mask = Image::frombytes("L", (5, 3), mask_bytes).unwrap();
    let mut expected_image = Image::frombytes(mode, (5, 3), destination_bytes).unwrap();
    expected_image
        .paste_at(
            PasteSource::Image(Box::new(source.clone())),
            Some(position),
            Some(&mask),
        )
        .unwrap();
    let expected = expected_image
        .use_backend(Backend::Cpu)
        .tobytes()
        .expect("CPU masked Paste reference");

    let mut actual_image = Image::frombytes(mode, (5, 3), destination_bytes).unwrap();
    actual_image
        .paste_at(
            PasteSource::Image(Box::new(source)),
            Some(position),
            Some(&mask),
        )
        .unwrap();
    let actual = actual_image
        .use_backend(Backend::Gpu)
        .tobytes()
        .expect("native GPU masked Paste");
    assert_eq!(actual, expected, "masked {mode} Paste at {position:?}");
    assert_native_masked_paste_receipt();
}

#[cfg(target_endian = "little")]
fn assert_native_masked_paste_receipt() {
    let receipt = Backend::take_pipeline_telemetry()
        .expect("native masked Paste must publish a telemetry receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("native masked GPU resources");
    assert_eq!(resources.mode_conversion_count, 0);
    assert_eq!(resources.upload_bytes, 48 + 48 + 16);
    assert_eq!(resources.auxiliary_bytes, 48 + 16);
    assert_eq!(resources.readback_bytes, 48);

    let shader_dispatches = Backend::take_gpu_shader_coverage();
    assert_eq!(shader_dispatches.len(), 1);
    assert_eq!(
        shader_dispatches[0].variant_name,
        "__internal_paste_native_masked_rgb"
    );
    assert_eq!(
        shader_dispatches[0].shader_file,
        "paste_native_masked_rgb.wgsl"
    );
    assert_eq!(shader_dispatches[0].dispatches, 1);
    assert_eq!(shader_dispatches[0].workgroups, 1);
}
