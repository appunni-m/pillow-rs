//! Geometry operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, execute_crop, execute_reduce, execute_resize,
    execute_resize_boxed, execute_rotate, execute_thumbnail, execute_transpose,
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
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Crop {
                    left,
                    top,
                    right,
                    bottom,
                } = op
                {
                    execute_crop(img, *left, *top, *right, *bottom)
                } else {
                    Err(PilError::ValueError("expected Crop op".into()))
                }
            },
            "crop.wgsl"
        ),
    );

    m.insert(
        "Rotate",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Rotate {
                    angle,
                    expand,
                    fill,
                    center,
                    translate,
                    filter,
                    nearest,
                } = op
                {
                    execute_rotate(
                        img, *angle, *expand, *fill, *center, *translate, *filter, *nearest, mode,
                    )
                } else {
                    Err(PilError::ValueError("expected Rotate op".into()))
                }
            },
            "rotate.wgsl"
        ),
    );
}

fn register_part_2(m: &mut Registry) {
    m.insert(
        "Transpose",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Transpose { method } = op {
                    execute_transpose(img, method)
                } else {
                    Err(PilError::ValueError("expected Transpose op".into()))
                }
            },
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
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Reduce { x_factor, y_factor } = op {
                    execute_reduce(img, *x_factor, *y_factor, mode)
                } else {
                    Err(PilError::ValueError("expected Reduce op".into()))
                }
            },
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
