//! Extract band operation registration.
use super::prelude::{DynamicImage, PilError, PipelineOp, Registry, op_extract_band};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "ExtractBand",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::ExtractBand { index } = op {
                    op_extract_band(img, *index)
                } else {
                    Err(PilError::ValueError("expected ExtractBand op".into()))
                }
            },
            "extract_band.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    Ok(())
}
