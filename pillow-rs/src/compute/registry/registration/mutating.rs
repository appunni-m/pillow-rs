//! Mutating operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, op_color3dlut, op_put_alpha, op_put_alpha_data,
    op_put_data, op_put_pixel,
};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "PutPixel",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutPixel { x, y, color, .. } = op {
                    op_put_pixel(img, *x, *y, *color)
                } else {
                    Err(PilError::ValueError("expected PutPixel op".into()))
                }
            },
            "put_pixel.wgsl"
        ),
    );

    m.insert(
        "PutData",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutData {
                    data,
                    mode: data_mode,
                } = op
                {
                    op_put_data(img, data, *data_mode)
                } else {
                    Err(PilError::ValueError("expected PutData op".into()))
                }
            },
            "put_data.wgsl"
        ),
    );

    m.insert(
        "Color3DLut",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Color3DLut {
                    size,
                    table,
                    channels,
                    source_mode,
                    target_mode,
                } = op
                {
                    op_color3dlut(img, *size, table, *channels, *source_mode, *target_mode)
                } else {
                    Err(PilError::ValueError("expected Color3DLut op".into()))
                }
            },
            "color_3dlut.wgsl"
        ),
    );
}

fn register_part_2(m: &mut Registry) {
    m.insert(
        "PutAlpha",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutAlpha {
                    alpha,
                    mode: alpha_mode,
                } = op
                {
                    Ok(op_put_alpha(img, *alpha, *alpha_mode))
                } else {
                    Err(PilError::ValueError("expected PutAlpha op".into()))
                }
            },
            "put_alpha.wgsl"
        ),
    );

    m.insert(
        "PutAlphaData",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::PutAlphaData { mask, mode } = op {
                    Ok(op_put_alpha_data(img, mask, *mode))
                } else {
                    Err(PilError::ValueError("expected PutAlphaData op".into()))
                }
            },
            "put_alpha_data.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    register_part_2(m);
    Ok(())
}
