//! Transform operation registration.
use super::prelude::{DynamicImage, PilError, PipelineOp, Registry, op_transform};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "Transform",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Transform {
                    w,
                    h,
                    method,
                    data,
                    filter,
                    fill,
                    fill_is_none,
                    palette_fill,
                } = op
                {
                    let resolved_fill = palette_fill.map(|index| (index, 0, 0, 255)).or(*fill);
                    op_transform(
                        img,
                        *w,
                        *h,
                        method,
                        data,
                        filter,
                        resolved_fill,
                        *fill_is_none,
                        mode,
                    )
                } else {
                    Err(PilError::ValueError("expected Transform op".into()))
                }
            },
            "transform.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    Ok(())
}
