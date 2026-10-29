//! Allocation receipts for native and fallback grayscale conversion.

use crate::color::pil_grayscale;
use crate::compute::Backend;
use crate::error::PilError;
use crate::raster::{DynamicImage, GrayAlphaImage, GrayImage, ImageBuffer, RgbImage, RgbaImage};

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

fn assert_grayscale(
    image: &DynamicImage,
    expected: &[u8],
    allocation_count: u64,
    allocated_bytes: u64,
) -> Result<(), PilError> {
    Backend::reset_pipeline_allocation_telemetry();
    let output = pil_grayscale(image)?;
    let allocations = Backend::take_pipeline_allocation_telemetry();

    assert_eq!(output.as_raw(), expected);
    assert_eq!(allocations.allocation_count, allocation_count);
    assert_eq!(allocations.allocated_bytes, allocated_bytes);
    Ok(())
}

#[test]
fn native_grayscale_layouts_record_one_output_allocation() -> Result<(), PilError> {
    let _telemetry = AllocationTelemetryGuard::enable();

    let luma = DynamicImage::ImageLuma8(GrayImage::from_pixel(
        WIDTH,
        HEIGHT,
        crate::raster::Luma([11]),
    ));
    assert_grayscale(&luma, &vec![11; PIXELS], 1, 51)?;

    let la = DynamicImage::ImageLumaA8(GrayAlphaImage::from_pixel(
        WIDTH,
        HEIGHT,
        crate::raster::LumaA([11, 73]),
    ));
    assert_grayscale(&la, &vec![11; PIXELS], 1, 51)?;

    let rgb = DynamicImage::ImageRgb8(RgbImage::from_pixel(
        WIDTH,
        HEIGHT,
        crate::raster::Rgb([10, 20, 30]),
    ));
    assert_grayscale(&rgb, &vec![18; PIXELS], 1, 51)?;

    let rgba = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        WIDTH,
        HEIGHT,
        crate::raster::Rgba([10, 20, 30, 73]),
    ));
    assert_grayscale(&rgba, &vec![18; PIXELS], 1, 51)?;
    Ok(())
}

#[test]
fn empty_native_layouts_record_no_grayscale_allocations() -> Result<(), PilError> {
    let _telemetry = AllocationTelemetryGuard::enable();
    let cases = [
        DynamicImage::ImageLuma8(GrayImage::new(0, HEIGHT)),
        DynamicImage::ImageLumaA8(GrayAlphaImage::new(0, HEIGHT)),
        DynamicImage::ImageRgb8(RgbImage::new(0, HEIGHT)),
        DynamicImage::ImageRgba8(RgbaImage::new(0, HEIGHT)),
    ];

    for image in cases {
        Backend::reset_pipeline_allocation_telemetry();
        let output = pil_grayscale(&image)?;
        let allocations = Backend::take_pipeline_allocation_telemetry();
        assert!(output.as_raw().is_empty());
        assert_eq!(allocations.allocation_count, 0);
        assert_eq!(allocations.allocated_bytes, 0);
    }
    Ok(())
}

#[test]
fn luma16_grayscale_fallback_records_rgb_and_gray_buffers() -> Result<(), PilError> {
    let _telemetry = AllocationTelemetryGuard::enable();
    let source = DynamicImage::ImageI16(ImageBuffer::from_fn(WIDTH, HEIGHT, |x, _| {
        crate::raster::Luma([if x % 2 == 0 { 0 } else { u16::MAX }])
    }));
    let expected: Vec<u8> = (0..HEIGHT)
        .flat_map(|_| (0..WIDTH).map(|x| if x % 2 == 0 { 0 } else { 255 }))
        .collect();

    assert_grayscale(&source, &expected, 2, 204)
}
