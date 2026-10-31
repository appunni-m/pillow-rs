//! AlphaComposite operation registration.

use super::prelude::{PilError, Registry};

pub(super) fn register(entries: &mut Registry) -> Result<(), PilError> {
    entries.insert(
        "AlphaComposite",
        gpu_entry!(
            crate::compute::pool_cpu::ops::composite::ops::prepare_pipeline,
            "alpha_composite.wgsl"
        ),
    );
    Ok(())
}

