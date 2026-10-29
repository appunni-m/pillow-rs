//! Geometry operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, execute_resize,
    execute_resize_boxed, execute_thumbnail,
};
fn register_resize(m: &mut Registry) {
    m.insert(
        "Resize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                match op {
                    PipelineOp::Resize { w, h, filter } => {
                        execute_resize(img, *w, *h, filter, mode)
                    }
                    PipelineOp::ResizeBoxed {
                        w,
                        h,
                        filter,
                        box_coords,
                    } => execute_resize_boxed(img, *w, *h, *filter, *box_coords, mode),
                    _ => Err(PilError::ValueError("expected Resize op".into())),
                }
            },
            "resize_nearest.wgsl"
        ),
    );
}

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
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Thumbnail { w, h, filter } = op {
                    execute_thumbnail(img, *w, *h, filter, mode)
                } else {
                    Err(PilError::ValueError("expected Thumbnail op".into()))
                }
            },
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

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_resize(m);
    register_crop_rotate(m);
    register_part_2(m);
    Ok(())
}
