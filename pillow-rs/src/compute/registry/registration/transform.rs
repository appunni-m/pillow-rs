//! Transform operation registration.
use super::prelude::{PilError, Registry};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "Transform",
        gpu_entry!(
            crate::compute::pool_cpu::ops::transform::ops::prepare_pipeline,
            "transform.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    Ok(())
}
