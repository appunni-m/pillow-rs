use super::super::{Image, PutDataValue};
use crate::error::PilError;

#[test]
fn cpu_known_zero_cmyk_projection_uses_sparse_writes() -> Result<(), PilError> {
    use crate::compute::Backend;

    let mut image = Image::new(7, 3, "CMYK", (0, 0, 0, 0))?;
    image.putpixel(1, 0, 1, 0, 0, 0)?;
    image.putpixel(4, 1, 0, 1, 0, 0)?;
    image.putpixel(6, 2, 0, 0, 0, 1)?;
    assert!(matches!(
        &image,
        Image::Pipeline { source, .. }
            if matches!(source.as_ref(), Image::Loaded(data) if data.known_zero_fill)
    ));
    image = image.use_backend(Backend::Cpu);

    assert_eq!(
        image.getprojection()?,
        (vec![0, 1, 0, 0, 1, 0, 1], vec![1, 1, 1])
    );
    Ok(())
}

#[test]
fn eager_pixel_data_mutation_invalidates_known_zero_source() -> Result<(), PilError> {
    use crate::compute::Backend;

    let mut image = Image::new(4, 1, "CMYK", (0, 0, 0, 0))?;
    assert!(matches!(
        &image,
        Image::Loaded(data) if data.known_zero_fill
    ));
    image.putdata_value_at(1, &PutDataValue::Components(vec![1, 0, 0, 0]), 1.0, 0.0)?;
    assert!(matches!(
        &image,
        Image::Loaded(data) if !data.known_zero_fill
    ));
    image.putpixel(0, 0, 0, 0, 0, 0)?;
    image = image.use_backend(Backend::Cpu);

    assert_eq!(image.getprojection()?, (vec![0, 1, 0, 0], vec![1]));
    Ok(())
}

#[test]
fn packed_cmyk_projection_covers_grouped_pixels_and_row_tails() -> Result<(), PilError> {
    let mut image = Image::new(7, 3, "CMYK", (0, 0, 0, 0))?;
    image.putpixel(1, 0, 1, 0, 0, 0)?;
    image.putpixel(4, 1, 0, 1, 0, 0)?;
    image.putpixel(6, 2, 0, 0, 1, 0)?;
    image.putpixel(2, 2, 0, 0, 0, 1)?;

    assert_eq!(
        image.getprojection()?,
        (vec![0, 1, 1, 0, 1, 0, 1], vec![1, 1, 1])
    );
    Ok(())
}

#[test]
fn packed_cmyk_projection_preserves_empty_width_rows() -> Result<(), PilError> {
    let image = Image::new(0, 3, "CMYK", (0, 0, 0, 0))?;
    assert_eq!(image.getprojection()?, (vec![], vec![0, 0, 0]));
    Ok(())
}

#[test]
fn pending_cmyk_projection_applies_overwrites_without_changing_image() -> Result<(), PilError> {
    let mut image = Image::new(7, 3, "CMYK", (1, 0, 0, 0))?;
    for x in 0..7 {
        image.putpixel(x, 2, 0, 0, 0, 0)?;
    }
    for y in 0..2 {
        image.putpixel(4, y, 0, 0, 0, 0)?;
    }
    image.putpixel(3, 0, 0, 0, 0, 0)?;
    image.putpixel(3, 0, 1, 0, 0, 0)?;

    assert_eq!(
        image.getprojection()?,
        (vec![1, 1, 1, 1, 0, 1, 1], vec![1, 1, 0])
    );

    let mut expected = Vec::with_capacity(7 * 3 * 4);
    for y in 0..3 {
        for x in 0..7 {
            expected.extend_from_slice(if y == 2 || x == 4 {
                &[0, 0, 0, 0]
            } else {
                &[1, 0, 0, 0]
            });
        }
    }
    assert_eq!(image.tobytes()?, expected);
    Ok(())
}

#[test]
fn simd_pending_cmyk_projection_fuses_writes_and_reports_vector_execution() -> Result<(), PilError>
{
    use crate::compute::Backend;

    let mut image = Image::new(65, 3, "CMYK", (0, 0, 0, 0))?;
    image.putpixel(1, 0, 1, 0, 0, 0)?;
    image.putpixel(1, 0, 0, 0, 0, 0)?;
    image.putpixel(64, 1, 0, 1, 0, 0)?;
    image.putpixel(3, 2, 0, 0, 0, 1)?;
    let image = image.use_backend(Backend::Simd);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let projection = image.getprojection()?;
    let receipt = Backend::take_pipeline_telemetry();
    let operation_receipts = Backend::take_pipeline_operation_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    let mut expected_horizontal = vec![0; 65];
    expected_horizontal[3] = 1;
    expected_horizontal[64] = 1;
    assert_eq!(projection, (expected_horizontal, vec![0, 1, 1]));
    let receipt = receipt.expect("SIMD terminal receipt");
    assert_eq!(receipt.1, Backend::Simd);
    assert_eq!(receipt.2, 4);
    assert_eq!(receipt.7, None);
    assert_eq!(operation_receipts.len(), 4);
    assert!(operation_receipts[0].vector_block_count > 0);
    Ok(())
}

#[test]
fn simd_known_zero_cmyk_projection_vectorizes_positive_writes_and_axis_tails()
-> Result<(), PilError> {
    use crate::compute::Backend;

    let mut image = Image::new(65, 3, "CMYK", (0, 0, 0, 0))?;
    image.putpixel(1, 0, 1, 0, 0, 0)?;
    image.putpixel(1, 0, 0, 1, 0, 0)?;
    image.putpixel(64, 2, 0, 0, 0, 1)?;
    let image = image.use_backend(Backend::Simd);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let projection = image.getprojection()?;
    let receipt = Backend::take_pipeline_telemetry();
    let operation_receipts = Backend::take_pipeline_operation_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    let mut expected_horizontal = vec![0; 65];
    expected_horizontal[1] = 1;
    expected_horizontal[64] = 1;
    let mut expected_vertical = vec![0; 3];
    expected_vertical[0] = 1;
    expected_vertical[2] = 1;
    assert_eq!(projection, (expected_horizontal, expected_vertical));
    let receipt = receipt.expect("SIMD terminal receipt");
    assert_eq!(receipt.1, Backend::Simd);
    assert_eq!(receipt.2, 3);
    assert_eq!(receipt.7, None);
    assert_eq!(operation_receipts.len(), 3);
    assert_eq!(operation_receipts[0].vector_block_count, 16);
    Ok(())
}

#[test]
fn simd_known_zero_cmyk_projection_scans_when_write_limit_is_exceeded() -> Result<(), PilError> {
    use crate::compute::Backend;

    let mut image = Image::new(9, 1, "CMYK", (0, 0, 0, 0))?;
    for x in 0..9 {
        image.putpixel(x, 0, 0, 0, 1, 0)?;
    }
    let image = image.use_backend(Backend::Simd);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let projection = image.getprojection()?;
    let receipt = Backend::take_pipeline_telemetry();
    let operation_receipts = Backend::take_pipeline_operation_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    assert_eq!(projection, (vec![1; 9], vec![1]));
    let receipt = receipt.expect("SIMD terminal receipt");
    assert_eq!(receipt.1, Backend::Simd);
    assert_eq!(receipt.2, 9);
    assert_eq!(receipt.7, None);
    assert_eq!(operation_receipts[0].vector_block_count, 2);
    Ok(())
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_pending_cmyk_projection_applies_zero_overwrites_across_wide_row_boundaries()
-> Result<(), PilError> {
    use crate::compute::Backend;

    let source = crate::raster::DynamicImage::ImageRgba8(crate::raster::RgbaImage::from_pixel(
        515,
        19,
        crate::raster::Rgba([0; 4]),
    ));
    let ops = [
        crate::pipeline::PipelineOp::PutPixel {
            x: 255,
            y: 16,
            color: (1, 0, 0, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 255,
            y: 16,
            color: (0, 0, 0, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 0,
            y: 0,
            color: (0, 0, 1, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 0,
            y: 0,
            color: (0, 0, 0, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 257,
            y: 7,
            color: (0, 1, 0, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 514,
            y: 18,
            color: (0, 0, 0, 1),
            palette_index: false,
        },
    ];
    if !crate::compute::gpu_cmyk_projection_terminal_supported(&source, &ops) {
        return Ok(());
    }

    let mut image = Image::new(515, 19, "CMYK", (0, 0, 0, 0))?;
    image.putpixel(255, 16, 1, 0, 0, 0)?;
    image.putpixel(255, 16, 0, 0, 0, 0)?;
    image.putpixel(0, 0, 0, 0, 1, 0)?;
    image.putpixel(0, 0, 0, 0, 0, 0)?;
    image.putpixel(257, 7, 0, 1, 0, 0)?;
    image.putpixel(514, 18, 0, 0, 0, 1)?;
    image = image.use_backend(Backend::Gpu);

    assert_gpu_pending_cmyk_projection_result(image, ops.len())
}

#[cfg(feature = "gpu")]
fn assert_gpu_pending_cmyk_projection_result(
    image: Image,
    operation_count: usize,
) -> Result<(), PilError> {
    use crate::compute::Backend;

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let result = image.getprojection();
    let receipt = Backend::take_pipeline_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    let (horizontal, vertical) = result?;
    let mut expected_horizontal = vec![0; 515];
    expected_horizontal[257] = 1;
    expected_horizontal[514] = 1;
    let mut expected_vertical = vec![0; 19];
    expected_vertical[7] = 1;
    expected_vertical[18] = 1;
    assert_eq!(
        (horizontal, vertical),
        (expected_horizontal, expected_vertical)
    );

    let receipt = receipt.expect("GPU projection terminal receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.2, operation_count);
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    let resources = receipt.8.expect("GPU projection resources");
    assert_eq!(resources.upload_bytes, 0);
    assert_eq!(resources.readback_bytes, 72);
    assert_eq!(resources.full_frame_copy_count, 0);
    Ok(())
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_pending_cmyk_projection_applies_overwrites_to_nonzero_source() -> Result<(), PilError> {
    use crate::compute::Backend;

    let (width, height) = (515, 19);
    let mut bytes = vec![0; width as usize * height as usize * 4];
    let source_offset = (3 * width as usize + 100) * 4;
    bytes[source_offset] = 1;
    let source = crate::raster::DynamicImage::ImageRgba8(
        crate::raster::RgbaImage::from_raw(width, height, bytes.clone())
            .expect("source carrier dimensions"),
    );
    let ops = [
        crate::pipeline::PipelineOp::PutPixel {
            x: 100,
            y: 3,
            color: (0, 0, 0, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 257,
            y: 7,
            color: (0, 1, 0, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 514,
            y: 18,
            color: (0, 0, 0, 1),
            palette_index: false,
        },
    ];
    if !crate::compute::gpu_cmyk_projection_terminal_supported(&source, &ops) {
        return Ok(());
    }

    let mut image = Image::frombytes("CMYK", (width, height), &bytes)?;
    image.putpixel(100, 3, 0, 0, 0, 0)?;
    image.putpixel(257, 7, 0, 1, 0, 0)?;
    image.putpixel(514, 18, 0, 0, 0, 1)?;
    image = image.use_backend(Backend::Gpu);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let result = image.getprojection();
    let receipt = Backend::take_pipeline_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    let (horizontal, vertical) = result?;
    let mut expected_horizontal = vec![0; width as usize];
    expected_horizontal[257] = 1;
    expected_horizontal[514] = 1;
    let mut expected_vertical = vec![0; height as usize];
    expected_vertical[7] = 1;
    expected_vertical[18] = 1;
    assert_eq!(
        (horizontal, vertical),
        (expected_horizontal, expected_vertical)
    );

    let receipt = receipt.expect("GPU projection terminal receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.2, ops.len());
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    Ok(())
}
