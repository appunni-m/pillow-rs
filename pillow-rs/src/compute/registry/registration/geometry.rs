//! Geometry operation registration.
use super::prelude::{PilError, Registry};
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn register_resize(m: &mut Registry) {
    m.insert(
        "Resize",
        gpu_entry!(
            crate::compute::pool_cpu::ops::resize::ops::prepare_pipeline,
            "resize_nearest.wgsl"
        ),
    );
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn register_crop_rotate(m: &mut Registry) {
    m.insert(
        "Crop",
        gpu_entry!(
            crate::compute::pool_cpu::ops::crop::ops::prepare_pipeline,
            "crop.wgsl"
        ),
    );

    m.insert(
        "Rotate",
        gpu_entry!(
            crate::compute::pool_cpu::ops::rotate::ops::prepare_pipeline,
            "rotate.wgsl"
        ),
    );
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
fn register_part_2(m: &mut Registry) {
    m.insert(
        "Transpose",
        gpu_entry!(
            crate::compute::pool_cpu::ops::transpose::ops::prepare_transpose_pipeline,
            "transpose.wgsl"
        ),
    );

    m.insert(
        "Thumbnail",
        gpu_entry!(
            crate::compute::pool_cpu::ops::thumbnail::ops::prepare_pipeline,
            "thumbnail.wgsl"
        ),
    );

    m.insert(
        "Reduce",
        gpu_entry!(
            crate::compute::pool_cpu::ops::reduce::ops::prepare_pipeline,
            "reduce.wgsl"
        ),
    );
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_resize(m);
    register_crop_rotate(m);
    register_part_2(m);
    Ok(())
}
