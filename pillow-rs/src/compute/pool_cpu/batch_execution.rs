//! Serial composition of prepared CPU operation strategies.

use crate::compute::registry::support::{OperationInputs, PaletteInput};
use crate::compute::{
    Architecture, Backend, ExecutionContext, account_host_buffer_boundary,
    begin_pipeline_operation_telemetry, finish_pipeline_operation_telemetry, host_resource_telemetry,
    record_pipeline_operation_path,
};
use crate::error::PilError;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

/// Prepare and execute each operation in order, preserving the mode returned
/// by each operation as the next operation's typed input identity.
pub(super) fn execute_batch(
    ops: &[PipelineOp],
    image: &DynamicImage,
    context: &ExecutionContext,
    palette: PaletteInput<'_>,
) -> Result<DynamicImage, PilError> {
    if context.backend() != Backend::Cpu {
        return Err(PilError::InternalError(
            "CPU batch execution received a non-CPU context".into(),
        ));
    }

    let mut result = None;
    let mut resources = host_resource_telemetry(image);
    for op in ops {
        let input = result.as_ref().unwrap_or(image);
        begin_pipeline_operation_telemetry(crate::compute::registry::variant_key(op));
        let operation_palette = match input.mode() {
            crate::raster::Mode::ImagePalette8 | crate::raster::Mode::ImagePaletteAlpha8 => {
                palette
            }
            _ => PaletteInput::default(),
        };
        let inputs = OperationInputs {
            image: input,
            palette: operation_palette,
        };
        let prepared = match crate::compute::registry::prepare_cpu(op, inputs, context) {
            Ok(prepared) => prepared,
            Err(error) => {
                record_pipeline_operation_path("cpu");
                finish_pipeline_operation_telemetry();
                return Err(error);
            }
        };
        let uses_default =
            prepared.selection().implementation.architecture == Architecture::Default;
        let output = match prepared.execute() {
            Ok(output) => output,
            Err(error) => {
                if uses_default {
                    record_pipeline_operation_path("cpu");
                }
                finish_pipeline_operation_telemetry();
                return Err(error);
            }
        };
        if uses_default {
            record_pipeline_operation_path("cpu");
        }
        finish_pipeline_operation_telemetry();
        account_host_buffer_boundary(&mut resources, input, &output);
        result = Some(output);
    }

    let output = result.unwrap_or_else(|| image.clone());
    crate::compute::record_pipeline_resource_telemetry(resources);
    Ok(output)
}

// TODO(pillow-rs#3ptb): restore the legacy executor's source-accounted
// operation fusion and terminal paths as prepared CPU helpers, preserving the
// same injected context and allocation boundaries.
