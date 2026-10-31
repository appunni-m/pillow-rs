//! ImageChops operation registration.

use super::prelude::{PilError, Registry};

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    m.insert("Add", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "add.wgsl"));
    m.insert("Subtract", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "subtract.wgsl"));
    m.insert("Multiply", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "multiply.wgsl"));
    m.insert("Screen", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "screen.wgsl"));
    m.insert("Darker", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "darker.wgsl"));
    m.insert("Lighter", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "lighter.wgsl"));
    m.insert("Difference", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "difference.wgsl"));
    m.insert("Overlay", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "overlay.wgsl"));
    m.insert("HardLight", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "hard_light.wgsl"));
    m.insert("SoftLight", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "soft_light.wgsl"));
    m.insert("AddModulo", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "add_modulo.wgsl"));
    m.insert("SubtractModulo", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "subtract_modulo.wgsl"));
    m.insert("LogicalAnd", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "logical_and.wgsl"));
    m.insert("LogicalOr", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "logical_or.wgsl"));
    m.insert("LogicalXor", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "logical_xor.wgsl"));
    m.insert("Constant", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "constant.wgsl"));
    m.insert("Offset", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "offset.wgsl"));
    m.insert("Duplicate", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "duplicate.wgsl"));
    m.insert("InvertChops", gpu_entry!(crate::compute::pool_cpu::ops::chops::ops::prepare_pipeline, "invert_chops.wgsl"));
    Ok(())
}
