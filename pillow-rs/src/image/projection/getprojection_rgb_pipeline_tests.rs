use super::super::{Image, PilError};

#[test]
fn cpu_zero_rgb_source_preserves_sparse_overwrite_order() -> Result<(), PilError> {
    use crate::compute::Backend;

    let mut image = Image::new(17, 4, "RGB", (0, 0, 0, 255))?;
    image.putpixel(0, 0, 1, 0, 0, 0)?;
    image.putpixel(16, 3, 0, 1, 0, 0)?;
    image.putpixel(0, 0, 0, 0, 0, 255)?;
    image = image.use_backend(Backend::Cpu);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let projection = image.getprojection()?;
    let receipt = Backend::take_pipeline_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    let mut expected_horizontal = vec![0; 17];
    expected_horizontal[16] = 1;
    assert_eq!((expected_horizontal, vec![0, 0, 0, 1]), projection);
    let receipt = receipt.expect("CPU sparse RGB projection terminal receipt");
    assert_eq!(receipt.0, Some(Backend::Cpu));
    assert_eq!(receipt.1, Backend::Cpu);
    assert_eq!(receipt.2, 3);
    assert_eq!(receipt.7, None);
    assert_eq!(
        receipt
            .8
            .expect("CPU terminal resource receipt")
            .fused_operation_count,
        3
    );
    Ok(())
}

#[test]
fn cpu_pending_rgb_projection_applies_overwrites_and_ignores_alpha() -> Result<(), PilError> {
    use crate::compute::Backend;

    let mut image = Image::new(7, 4, "RGB", (1, 0, 0, 255))?;
    for x in 0..7 {
        image.putpixel(x, 3, 0, 0, 0, 255)?;
    }
    for y in 0..3 {
        image.putpixel(4, y, 0, 0, 0, 255)?;
    }
    image.putpixel(3, 0, 0, 0, 0, 255)?;
    image.putpixel(3, 0, 0, 1, 0, 0)?;
    image.putpixel(6, 3, 0, 0, 0, 255)?;
    image = image.use_backend(Backend::Cpu);

    assert_eq!(
        image.getprojection()?,
        (vec![1, 1, 1, 1, 0, 1, 1], vec![1, 1, 1, 0])
    );
    Ok(())
}

#[test]
fn simd_pending_rgb_projection_executes_vector_blocks() -> Result<(), PilError> {
    use crate::compute::Backend;

    let mut image = Image::new(17, 4, "RGB", (0, 0, 0, 255))?;
    image.putpixel(0, 0, 1, 0, 0, 0)?;
    image.putpixel(16, 3, 0, 1, 0, 0)?;
    image.putpixel(4, 2, 0, 0, 0, 255)?;
    image = image.use_backend(Backend::Simd);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let projection = image.getprojection()?;
    let receipt = Backend::take_pipeline_telemetry();
    let operation_receipts = Backend::take_pipeline_operation_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    let mut expected_horizontal = vec![0; 17];
    expected_horizontal[0] = 1;
    expected_horizontal[16] = 1;
    assert_eq!(projection, (expected_horizontal, vec![1, 0, 0, 1]));
    let receipt = receipt.expect("SIMD RGB projection terminal receipt");
    assert_eq!(receipt.1, Backend::Simd);
    assert_eq!(receipt.2, 3);
    assert_eq!(receipt.7, None);
    assert_eq!(operation_receipts.len(), 3);
    assert!(operation_receipts[0].vector_block_count > 0);
    Ok(())
}

#[cfg(feature = "gpu")]
#[test]
fn gpu_pending_rgb_projection_executes_terminal_and_ignores_alpha() -> Result<(), PilError> {
    use crate::compute::Backend;

    let source = crate::raster::DynamicImage::ImageRgb8(crate::raster::RgbImage::from_pixel(
        17,
        4,
        crate::raster::Rgb([0; 3]),
    ));
    let ops = [
        crate::pipeline::PipelineOp::PutPixel {
            x: 0,
            y: 0,
            color: (1, 0, 0, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 16,
            y: 3,
            color: (0, 1, 0, 0),
            palette_index: false,
        },
        crate::pipeline::PipelineOp::PutPixel {
            x: 4,
            y: 2,
            color: (0, 0, 0, 255),
            palette_index: false,
        },
    ];
    if !crate::compute::gpu_rgb_projection_terminal_supported(&source, &ops) {
        return Ok(());
    }

    let mut image = Image::new(17, 4, "RGB", (0, 0, 0, 255))?;
    image.putpixel(0, 0, 1, 0, 0, 0)?;
    image.putpixel(16, 3, 0, 1, 0, 0)?;
    image.putpixel(4, 2, 0, 0, 0, 255)?;
    image = image.use_backend(Backend::Gpu);

    let previous = Backend::set_pipeline_telemetry_enabled(true);
    let result = image.getprojection();
    let receipt = Backend::take_pipeline_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    let (horizontal, vertical) = result?;
    let mut expected_horizontal = vec![0; 17];
    expected_horizontal[0] = 1;
    expected_horizontal[16] = 1;
    assert_eq!(
        (horizontal, vertical),
        (expected_horizontal, vec![1, 0, 0, 1])
    );
    let receipt = receipt.expect("GPU RGB projection terminal receipt");
    assert_eq!(receipt.0, Some(Backend::Gpu));
    assert_eq!(receipt.1, Backend::Gpu);
    assert_eq!(receipt.2, ops.len());
    assert_eq!(receipt.6, Some(1));
    assert_eq!(receipt.7, None);
    Ok(())
}
