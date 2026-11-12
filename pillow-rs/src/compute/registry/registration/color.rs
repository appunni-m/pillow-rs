//! Color operation registration.
//!
use super::prelude::{PilError, Registry};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "Convert",
        gpu_entry!(
            crate::compute::pool_cpu::ops::color::ops::prepare_convert_pipeline,
            "convert.wgsl"
        ),
    );

    m.insert(
        "ConvertLab",
        gpu_entry!(
            crate::compute::pool_cpu::ops::color::ops::prepare_pipeline,
            "rgb_to_lab.wgsl"
        ),
    );

    m.insert(
        "RemapPalette",
        gpu_entry!(
            crate::compute::pool_cpu::ops::color::ops::prepare_remap_palette_pipeline,
            "remap_palette.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    Ok(())
}
