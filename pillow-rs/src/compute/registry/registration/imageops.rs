//! Imageops operation registration.
// TODO(pillow-rs#3ptb): replace the remaining direct-result callbacks for
// Contain, Cover, Fit, Pad, Scale, and Expand with operation-owned prepared
// plans. For Expand, retain the exact mode/output identity and P/PA raw-sample
// behavior when wiring its legacy native-byte paths. The two histogram
// operations now enter through their CPU plans.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, op_contain, op_cover,
    op_fit, op_pad, op_scale,
};
fn register_autocontrast_equalize(m: &mut Registry) {
    m.insert(
        "Autocontrast",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::autocontrast::prepare_pipeline,
            "autocontrast.wgsl"
        ),
    );

    m.insert(
        "Equalize",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::equalize::prepare_pipeline,
            "equalize.wgsl"
        ),
    );
}

fn register_invert_flip(m: &mut Registry) {
    m.insert(
        "Invert",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::ops::prepare_invert_pipeline,
            "invert.wgsl"
        ),
    );

    m.insert(
        "Flip",
        gpu_entry!(
            crate::compute::pool_cpu::ops::transpose::ops::prepare_flip_pipeline,
            "flip.wgsl"
        ),
    );
}

fn register_mirror_posterize_solarize(m: &mut Registry) {
    m.insert(
        "Mirror",
        gpu_entry!(
            crate::compute::pool_cpu::ops::transpose::ops::prepare_mirror_pipeline,
            "mirror.wgsl"
        ),
    );

    m.insert(
        "Posterize",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::ops::prepare_posterize_pipeline,
            "posterize.wgsl"
        ),
    );

    m.insert(
        "Solarize",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::ops::prepare_solarize_pipeline,
            "solarize.wgsl"
        ),
    );
}

fn register_grayscale(m: &mut Registry) {
    m.insert(
        "Grayscale",
        gpu_entry!(
            crate::compute::pool_cpu::ops::grayscale::ops::prepare_pipeline,
            "grayscale.wgsl"
        ),
    );
}

fn register_part_3(m: &mut Registry) {
    m.insert(
        "Colorize",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::ops::prepare_colorize_pipeline,
            "colorize.wgsl"
        ),
    );

    m.insert(
        "Contain",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Contain { w, h, filter } = op {
                    op_contain(img, *w, *h, *filter, mode)
                } else {
                    Err(PilError::ValueError("expected Contain op".into()))
                }
            },
            "contain.wgsl"
        ),
    );

    m.insert(
        "Cover",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Cover { w, h, filter } = op {
                    op_cover(img, *w, *h, *filter, mode)
                } else {
                    Err(PilError::ValueError("expected Cover op".into()))
                }
            },
            "cover.wgsl"
        ),
    );
}

fn register_fit_pad(m: &mut Registry) {
    m.insert(
        "Fit",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Fit {
                    w,
                    h,
                    filter,
                    bleed,
                    centering,
                } = op
                {
                    op_fit(img, *w, *h, *filter, *bleed, *centering, mode)
                } else {
                    Err(PilError::ValueError("expected Fit op".into()))
                }
            },
            "fit.wgsl"
        ),
    );

    m.insert(
        "Pad",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Pad {
                    w,
                    h,
                    filter,
                    color,
                    centering,
                } = op
                {
                    op_pad(img, *w, *h, *filter, *color, *centering, mode)
                } else {
                    Err(PilError::ValueError("expected Pad op".into()))
                }
            },
            "pad.wgsl"
        ),
    );
}

fn register_scale_expand(m: &mut Registry) {
    m.insert(
        "Scale",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Scale { factor, filter } = op {
                    op_scale(img, *factor, *filter, mode)
                } else {
                    Err(PilError::ValueError("expected Scale op".into()))
                }
            },
            "scale.wgsl"
        ),
    );

    m.insert(
        "Expand",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::ops::prepare_expand_pipeline,
            "expand.wgsl"
        ),
    );
}

fn register_part_5(m: &mut Registry) {
    m.insert(
        "CropBorder",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::ops::prepare_crop_border_pipeline,
            "crop_border.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_autocontrast_equalize(m);
    register_invert_flip(m);
    register_mirror_posterize_solarize(m);
    register_grayscale(m);
    register_part_3(m);
    register_fit_pad(m);
    register_scale_expand(m);
    register_part_5(m);
    Ok(())
}
