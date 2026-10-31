use super::{Backend, PilError, execute_prepared, prepare_execution};
use crate::compute::registry;
use crate::pipeline::{PipelineOp, TransposeMethod};
use crate::raster::{DynamicImage, GenericImageView, RgbaImage};

#[test]
fn fused_native_copy_preserves_avx2_transpose_receipt() {
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    crate::compute::reset_pipeline_operation_telemetry();

    crate::compute::begin_pipeline_operation_telemetry("Transpose");
    crate::compute::begin_pipeline_operation_telemetry("Transpose");
    crate::compute::record_pipeline_operation_vector_blocks(8);
    crate::compute::record_pipeline_operation_path("avx2-transpose");
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::finish_pipeline_operation_telemetry();
    crate::compute::record_pipeline_operation_path("native-copy");
    crate::compute::finish_pipeline_operation_telemetry();

    let records = crate::compute::take_pipeline_operation_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .any(|record| { record.path == "avx2-transpose" && record.vector_block_count == 8 })
    );
    assert!(
        records
            .iter()
            .any(|record| { record.path == "native-copy" && record.vector_block_count == 0 })
    );
}

#[test]
fn simd_compatibility_request_composes_consecutive_transposes() -> Result<(), PilError> {
    let source = DynamicImage::ImageRgba8(
        RgbaImage::from_raw(
            19,
            17,
            (0..19 * 17 * 4)
                .map(|index| (index * 73 + index / 11) as u8)
                .collect(),
        )
        .expect("RGBA source shape must be valid"),
    );
    let ops = [
        PipelineOp::Transpose {
            method: TransposeMethod::Transpose,
        },
        PipelineOp::Transpose {
            method: TransposeMethod::FlipLeftRight,
        },
    ];
    let expected = ops.iter().try_fold(source.clone(), |input, op| {
        registry::execute_cpu(op, &input, Some("RGBA"))
    })?;

    let prepared = prepare_execution(&ops, Some(Backend::Simd))?;
    let previous = Backend::set_pipeline_telemetry_enabled(true);
    crate::compute::reset_pipeline_operation_telemetry();
    let result = execute_prepared(
        &prepared,
        &ops,
        &source,
        Some(crate::raster::Mode::ImageRgba8),
    );
    let receipt = Backend::take_pipeline_telemetry();
    let operation_records = crate::compute::take_pipeline_operation_telemetry();
    Backend::set_pipeline_telemetry_enabled(previous);

    let actual = result?;
    assert_eq!(actual.as_bytes(), expected.as_bytes());
    assert_eq!(actual.dimensions(), expected.dimensions());
    let receipt = receipt.expect("CPU architecture execution receipt");
    assert_eq!(receipt.0, Some(Backend::Simd));
    assert_eq!(receipt.1, Backend::Cpu);
    assert_eq!(receipt.2, 2);
    assert_eq!(operation_records.len(), 2);
    assert!(operation_records
        .iter()
        .all(|record| record.selected_architecture.is_some()));
    Ok(())
}
