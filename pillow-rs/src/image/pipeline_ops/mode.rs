//! Lazy raster-mode output planning from operation support declarations.

use crate::compute::registry::support::{ModeSupport, OutputModeRule};
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

use super::super::Image;

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn target_mode_for_pipeline_op(op: &PipelineOp) -> Option<Mode> {
    match op {
        PipelineOp::Convert { mode, .. } | PipelineOp::Merge { mode, .. } => Some(*mode),
        PipelineOp::Color3DLut { target_mode, .. } => Some(*target_mode),
        _ => None,
    }
}

// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn output_support_mode(op: &PipelineOp, source: Mode) -> Mode {
    match op {
        // Merge declares support by its requested result identity; the first
        // band's mode is independent and can be L while the output is RGB.
        PipelineOp::Merge { mode, .. } => *mode,
        _ => source,
    }
}

/// Resolves a declared output rule against this operation's typed parameters.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn output_mode_from_rule(op: &PipelineOp, source: Mode, output: OutputModeRule) -> Option<Mode> {
    match output {
        OutputModeRule::NoRasterResult => None,
        OutputModeRule::Preserve => Some(source),
        OutputModeRule::Fixed(mode) => Some(mode),
        OutputModeRule::ByTargetMode | OutputModeRule::ByTargetModeWithPalette => {
            target_mode_for_pipeline_op(op)
        }
        OutputModeRule::ByPayload(outputs) => {
            let PipelineOp::PutData { payload_mode, .. } = op else {
                return None;
            };
            outputs
                .iter()
                .find(|output| output.payload == *payload_mode)
                .map(|output| output.output)
        }
    }
}

/// Resolves the success output or the operation-declared lazy output retained
/// when a rejected source mode reaches its existing wrapper error.
// Source audit (2026-11-01): every pipeline operation with a rejected source
// mode and a statically exposed changed output declares it here: Convert,
// ConvertMatrix, ConvertLab, RemapPalette, Grayscale, Colorize, Color3DLut,
// Merge, and EffectNoise. Other rejected routes either preserve source mode,
// produce no raster result (such as rejected Equalize), or are rejected by the
// public PutAlpha wrappers before queueing. A raw invalid `push_op` therefore
// keeps its materialization-time operation error instead of claiming a result
// mode. TODO(pillow-rs#3ptb): verify these metadata/error outcomes in frozen
// CPU parity; no runtime validation has run for this audit.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
fn output_mode_from_operation_contract(op: &PipelineOp, source: Mode) -> Option<Mode> {
    let support = op.support();
    let rule = match support.for_mode(output_support_mode(op, source)) {
        ModeSupport::Supported { output, .. } => output,
        ModeSupport::Rejected { .. } => support.rejected_output()?,
    };
    output_mode_from_rule(op, source, rule)
}

/// Plans a pipeline mode when no wrapper palette-expansion metadata is present.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn known_pipeline_op_mode(op: &PipelineOp, current: Mode) -> Option<Mode> {
    known_pipeline_op_mode_with_expansion(op, current, None)
}

/// Computes the exact statically known result identity for one pipeline op.
///
/// `None` preserves the materialization fallback when runtime data or error
/// order determines the result. Indexed safety is evaluated from the current
/// mode at each transition, not the pipeline's original source mode.
/// TODO(pillow-rs#3ptb): verify indexed output identity and fault timing in the
/// frozen CPU parity matrix.
// VERIFY: source=reviewed@2026-11-12; parity=pending; benchmark=pending
pub(crate) fn known_pipeline_op_mode_with_expansion(
    op: &PipelineOp,
    current: Mode,
    expanded_mode: Option<Mode>,
) -> Option<Mode> {
    if matches!(current, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
        && !Image::is_palette_safe_op_for_mode(current, op)
    {
        // Palette-dependent input expansion may change a preserving operation
        // from P/PA to RGB/RGBA. Fixed outputs and typed targets are independent
        // of that expansion; Preserve uses wrapper metadata when it is known.
        let support = op.support();
        let output = match support.for_mode(output_support_mode(op, current)) {
            ModeSupport::Supported { output, .. } => output,
            ModeSupport::Rejected { .. } if matches!(op, PipelineOp::Merge { .. }) => {
                support.rejected_output()?
            }
            ModeSupport::Rejected { .. } => return None,
        };
        return match output {
            OutputModeRule::NoRasterResult => None,
            OutputModeRule::Preserve => expanded_mode,
            fixed_or_target => output_mode_from_rule(op, current, fixed_or_target),
        };
    }
    output_mode_from_operation_contract(op, current)
}
