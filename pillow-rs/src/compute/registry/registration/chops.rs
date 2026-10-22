//! Chops operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, op_chops_add, op_chops_add_modulo,
    op_chops_constant, op_chops_darker, op_chops_difference, op_chops_duplicate,
    op_chops_hard_light, op_chops_invert, op_chops_lighter, op_chops_logical_and,
    op_chops_logical_or, op_chops_logical_xor, op_chops_multiply, op_chops_offset,
    op_chops_overlay, op_chops_screen, op_chops_soft_light, op_chops_subtract,
    op_chops_subtract_modulo,
};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "Add",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Add {
                    other,
                    scale,
                    offset,
                } = op
                {
                    op_chops_add(img, other, *scale, *offset)
                } else {
                    Err(PilError::ValueError("expected Add op".into()))
                }
            },
            "add.wgsl"
        ),
    );

    m.insert(
        "Subtract",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Subtract {
                    other,
                    scale,
                    offset,
                } = op
                {
                    op_chops_subtract(img, other, *scale, *offset)
                } else {
                    Err(PilError::ValueError("expected Subtract op".into()))
                }
            },
            "subtract.wgsl"
        ),
    );

    m.insert(
        "Multiply",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Multiply { other } = op {
                    op_chops_multiply(img, other)
                } else {
                    Err(PilError::ValueError("expected Multiply op".into()))
                }
            },
            "multiply.wgsl"
        ),
    );

    m.insert(
        "Screen",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Screen { other } = op {
                    op_chops_screen(img, other)
                } else {
                    Err(PilError::ValueError("expected Screen op".into()))
                }
            },
            "screen.wgsl"
        ),
    );

    m.insert(
        "Darker",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Darker { other } = op {
                    op_chops_darker(img, other)
                } else {
                    Err(PilError::ValueError("expected Darker op".into()))
                }
            },
            "darker.wgsl"
        ),
    );
}

fn register_part_2(m: &mut Registry) {
    m.insert(
        "Lighter",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Lighter { other } = op {
                    op_chops_lighter(img, other)
                } else {
                    Err(PilError::ValueError("expected Lighter op".into()))
                }
            },
            "lighter.wgsl"
        ),
    );

    m.insert(
        "Difference",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Difference { other } = op {
                    op_chops_difference(img, other)
                } else {
                    Err(PilError::ValueError("expected Difference op".into()))
                }
            },
            "difference.wgsl"
        ),
    );

    m.insert(
        "Overlay",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Overlay { other } = op {
                    op_chops_overlay(img, other)
                } else {
                    Err(PilError::ValueError("expected Overlay op".into()))
                }
            },
            "overlay.wgsl"
        ),
    );

    m.insert(
        "HardLight",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::HardLight { other } = op {
                    op_chops_hard_light(img, other)
                } else {
                    Err(PilError::ValueError("expected HardLight op".into()))
                }
            },
            "hard_light.wgsl"
        ),
    );

    m.insert(
        "SoftLight",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::SoftLight { other } = op {
                    op_chops_soft_light(img, other)
                } else {
                    Err(PilError::ValueError("expected SoftLight op".into()))
                }
            },
            "soft_light.wgsl"
        ),
    );
}

fn register_part_3(m: &mut Registry) {
    m.insert(
        "AddModulo",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::AddModulo { other } = op {
                    op_chops_add_modulo(img, other)
                } else {
                    Err(PilError::ValueError("expected AddModulo op".into()))
                }
            },
            "add_modulo.wgsl"
        ),
    );

    m.insert(
        "SubtractModulo",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::SubtractModulo { other } = op {
                    op_chops_subtract_modulo(img, other)
                } else {
                    Err(PilError::ValueError("expected SubtractModulo op".into()))
                }
            },
            "subtract_modulo.wgsl"
        ),
    );

    m.insert(
        "LogicalAnd",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::LogicalAnd { other } = op {
                    op_chops_logical_and(img, other)
                } else {
                    Err(PilError::ValueError("expected LogicalAnd op".into()))
                }
            },
            "logical_and.wgsl"
        ),
    );

    m.insert(
        "LogicalOr",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::LogicalOr { other } = op {
                    op_chops_logical_or(img, other)
                } else {
                    Err(PilError::ValueError("expected LogicalOr op".into()))
                }
            },
            "logical_or.wgsl"
        ),
    );

    m.insert(
        "LogicalXor",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::LogicalXor { other } = op {
                    op_chops_logical_xor(img, other)
                } else {
                    Err(PilError::ValueError("expected LogicalXor op".into()))
                }
            },
            "logical_xor.wgsl"
        ),
    );
}

fn register_part_4(m: &mut Registry) {
    m.insert(
        "Constant",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Constant { value } = op {
                    Ok(op_chops_constant(img, *value))
                } else {
                    Err(PilError::ValueError("expected Constant op".into()))
                }
            },
            "constant.wgsl"
        ),
    );

    m.insert(
        "Offset",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Offset { x, y } = op {
                    Ok(op_chops_offset(img, *x, *y, mode))
                } else {
                    Err(PilError::ValueError("expected Offset op".into()))
                }
            },
            "offset.wgsl"
        ),
    );

    m.insert(
        "Duplicate",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Duplicate) {
                    Ok(op_chops_duplicate(img))
                } else {
                    Err(PilError::ValueError("expected Duplicate op".into()))
                }
            },
            "duplicate.wgsl"
        ),
    );

    m.insert(
        "InvertChops",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::InvertChops) {
                    op_chops_invert(img)
                } else {
                    Err(PilError::ValueError("expected InvertChops op".into()))
                }
            },
            "invert_chops.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    register_part_2(m);
    register_part_3(m);
    register_part_4(m);
    Ok(())
}
