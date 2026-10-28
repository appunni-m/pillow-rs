//! Allocation receipts and alpha semantics for CPU LA conversion.

use super::op_convert;
use crate::compute::Backend;
use crate::error::PilError;
use crate::pipeline::ColorMode;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, RgbImage, RgbaImage};

const WIDTH: u32 = 17;
const HEIGHT: u32 = 3;
const PIXELS: usize = (WIDTH * HEIGHT) as usize;

struct AllocationTelemetryGuard {
    previous: bool,
}

impl AllocationTelemetryGuard {
    fn enable() -> Self {
        let previous = Backend::set_pipeline_telemetry_enabled(true);
        Self { previous }
    }
}

impl Drop for AllocationTelemetryGuard {
    fn drop(&mut self) {
        Backend::set_pipeline_telemetry_enabled(self.previous);
    }
}

fn repeated_la(luma: u8, alpha: u8) -> Vec<u8> {
    (0..PIXELS).flat_map(|_| [luma, alpha]).collect()
}

fn assert_la_conversion(
    image: &DynamicImage,
    source_mode: Option<&str>,
    expected: &[u8],
    allocation_count: u64,
    allocated_bytes: u64,
) -> Result<(), PilError> {
    Backend::reset_pipeline_allocation_telemetry();
    let output = op_convert(image, &ColorMode::LA, source_mode)?;
    let allocations = Backend::take_pipeline_allocation_telemetry();

    assert!(matches!(&output, DynamicImage::ImageLumaA8(_)));
    assert_eq!(output.as_bytes(), expected);
    assert_eq!(allocations.allocation_count, allocation_count);
    assert_eq!(allocations.allocated_bytes, allocated_bytes);
    Ok(())
}

#[test]
fn la_conversion_records_native_layout_allocations_and_preserves_alpha() -> Result<(), PilError> {
    let _telemetry = AllocationTelemetryGuard::enable();

    let luma = DynamicImage::ImageLuma8(GrayImage::from_pixel(
        WIDTH,
        HEIGHT,
        crate::raster::Luma([11]),
    ));
    assert_la_conversion(&luma, Some("L"), &repeated_la(11, 255), 2, 153)?;

    let native_la = DynamicImage::ImageLumaA8(GrayAlphaImage::from_fn(WIDTH, HEIGHT, |x, y| {
        let index = (y * WIDTH + x) as u8;
        crate::raster::LumaA([11, 40 + index])
    }));
    let expected_native_la: Vec<u8> = (0..PIXELS)
        .flat_map(|index| [11, 40 + index as u8])
        .collect();
    assert_la_conversion(&native_la, Some("LA"), &expected_native_la, 3, 204)?;
    assert_la_conversion(&native_la, None, &expected_native_la, 3, 204)?;

    let rgb = DynamicImage::ImageRgb8(RgbImage::from_pixel(
        WIDTH,
        HEIGHT,
        crate::raster::Rgb([10, 20, 30]),
    ));
    assert_la_conversion(&rgb, Some("RGB"), &repeated_la(18, 255), 2, 153)?;

    let rgba = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        WIDTH,
        HEIGHT,
        crate::raster::Rgba([10, 20, 30, 73]),
    ));
    assert_la_conversion(&rgba, Some("RGBA"), &repeated_la(18, 73), 3, 204)?;

    // RGBX shares RGBA storage but its fourth byte is padding, not alpha.
    assert_la_conversion(&rgba, Some("RGBX"), &repeated_la(18, 255), 2, 153)?;

    // PA is tested here as its internal two-byte transport only; this does
    // not model an expanded palette image or replace public PA parity cases.
    let pa_transport = DynamicImage::ImageLumaA8(GrayAlphaImage::from_pixel(
        WIDTH,
        HEIGHT,
        crate::raster::LumaA([11, 73]),
    ));
    assert_la_conversion(&pa_transport, Some("PA"), &repeated_la(11, 255), 2, 153)?;
    Ok(())
}

#[test]
fn empty_native_layouts_convert_to_la_without_buffer_allocations() -> Result<(), PilError> {
    let _telemetry = AllocationTelemetryGuard::enable();
    let cases = [
        (
            DynamicImage::ImageLuma8(GrayImage::new(0, HEIGHT)),
            Some("L"),
        ),
        (
            DynamicImage::ImageLumaA8(GrayAlphaImage::new(0, HEIGHT)),
            Some("LA"),
        ),
        (
            DynamicImage::ImageRgb8(RgbImage::new(0, HEIGHT)),
            Some("RGB"),
        ),
        (
            DynamicImage::ImageRgba8(RgbaImage::new(0, HEIGHT)),
            Some("RGBA"),
        ),
    ];

    for (image, source_mode) in cases {
        Backend::reset_pipeline_allocation_telemetry();
        let output = op_convert(&image, &ColorMode::LA, source_mode)?;
        let allocations = Backend::take_pipeline_allocation_telemetry();
        assert!(output.as_bytes().is_empty());
        assert_eq!(allocations.allocation_count, 0);
        assert_eq!(allocations.allocated_bytes, 0);
    }
    Ok(())
}

#[test]
fn overlong_alpha_storage_records_capacity_beyond_logical_pixels() -> Result<(), PilError> {
    let _telemetry = AllocationTelemetryGuard::enable();
    // Rust callers can retain complete extra pixels in ImageBuffer storage.
    // Existing alpha collectors process those bytes even though the returned
    // LA image contains only the logical frame. Count that real allocation.
    let stored_pixels = PIXELS + 15;
    let la_bytes: Vec<u8> = (0..stored_pixels)
        .flat_map(|index| [11, 40 + index as u8])
        .collect();
    let la = DynamicImage::ImageLumaA8(
        GrayAlphaImage::from_raw(WIDTH, HEIGHT, la_bytes).expect("valid overlong LA storage"),
    );
    let expected_la: Vec<u8> = (0..PIXELS)
        .flat_map(|index| [11, 40 + index as u8])
        .collect();
    assert_la_conversion(&la, Some("LA"), &expected_la, 3, 234)?;

    let rgba_bytes: Vec<u8> = (0..stored_pixels)
        .flat_map(|index| [10, 20, 30, 40 + index as u8])
        .collect();
    let rgba = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(WIDTH, HEIGHT, rgba_bytes).expect("valid overlong RGBA storage"),
    );
    let expected_rgba: Vec<u8> = (0..PIXELS)
        .flat_map(|index| [18, 40 + index as u8])
        .collect();
    assert_la_conversion(&rgba, Some("RGBA"), &expected_rgba, 3, 219)
}
