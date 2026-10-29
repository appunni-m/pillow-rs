//! Extract band operation registration.
use super::prelude::{PilError, Registry};

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    m.insert(
        "ExtractBand",
        gpu_entry!(
            crate::compute::pool_cpu::ops::put::ops::prepare_pipeline,
            "extract_band.wgsl"
        ),
    );
    Ok(())
}
