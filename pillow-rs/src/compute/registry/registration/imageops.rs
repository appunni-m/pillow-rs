//! Imageops operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, op_autocontrast, op_colorize, op_contain,
    op_cover, op_crop_border, op_equalize, op_equalize_with_mask, op_expand, op_fit, op_flip,
    op_invert, op_mirror, op_pad, op_posterize, op_scale, op_solarize,
};
fn register_autocontrast_equalize(m: &mut Registry) {
    m.insert(
        "Autocontrast",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let Some(m) = mode
                    && (m == "LA" || m == "RGBA")
                {
                    return Err(PilError::OsError(format!("not supported for mode {m}")));
                }
                if let PipelineOp::Autocontrast { cutoff, mask } = op {
                    op_autocontrast(img, *cutoff, mask.as_ref())
                } else {
                    Err(PilError::ValueError("expected Autocontrast op".into()))
                }
            },
            "autocontrast.wgsl"
        ),
    );

    m.insert(
        "Equalize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let Some(m) = mode
                    && (m == "LA" || m == "RGBA")
                {
                    return Err(PilError::OsError(format!("not supported for mode {m}")));
                }
                match op {
                    PipelineOp::Equalize => op_equalize(img),
                    PipelineOp::EqualizeMasked { mask } => op_equalize_with_mask(img, Some(mask)),
                    _ => Err(PilError::ValueError("expected Equalize op".into())),
                }
            },
            "equalize.wgsl"
        ),
    );
}

fn register_invert_flip(m: &mut Registry) {
    m.insert(
        "Invert",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Invert) {
                    op_invert(img)
                } else {
                    Err(PilError::ValueError("expected Invert op".into()))
                }
            },
            "invert.wgsl"
        ),
    );

    m.insert(
        "Flip",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Flip) {
                    op_flip(img, mode)
                } else {
                    Err(PilError::ValueError("expected Flip op".into()))
                }
            },
            "flip.wgsl"
        ),
    );
}

fn register_mirror_posterize_solarize(m: &mut Registry) {
    m.insert(
        "Mirror",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Mirror) {
                    op_mirror(img)
                } else {
                    Err(PilError::ValueError("expected Mirror op".into()))
                }
            },
            "mirror.wgsl"
        ),
    );

    m.insert(
        "Posterize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Posterize { bits } = op {
                    op_posterize(img, *bits)
                } else {
                    Err(PilError::ValueError("expected Posterize op".into()))
                }
            },
            "posterize.wgsl"
        ),
    );

    m.insert(
        "Solarize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Solarize { threshold } = op {
                    op_solarize(img, *threshold)
                } else {
                    Err(PilError::ValueError("expected Solarize op".into()))
                }
            },
            "solarize.wgsl"
        ),
    );
}

fn register_grayscale(m: &mut Registry) {
    m.insert(
        "Grayscale",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::Grayscale) {
                    crate::compute::pool_cpu::ops::grayscale::op_grayscale(img, mode)
                } else {
                    Err(PilError::ValueError("expected Grayscale op".into()))
                }
            },
            "grayscale.wgsl"
        ),
    );
}

fn register_part_3(m: &mut Registry) {
    m.insert(
        "Colorize",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Colorize {
                    black,
                    white,
                    mid,
                    blackpoint,
                    midpoint,
                    whitepoint,
                } = op
                {
                    op_colorize(img, black, white, *mid, *blackpoint, *midpoint, *whitepoint)
                } else {
                    Err(PilError::ValueError("expected Colorize op".into()))
                }
            },
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
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Expand { border, fill } = op {
                    op_expand(img, *border, *fill, mode)
                } else {
                    Err(PilError::ValueError("expected Expand op".into()))
                }
            },
            "expand.wgsl"
        ),
    );
}

fn register_part_5(m: &mut Registry) {
    m.insert(
        "CropBorder",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::CropBorder { border } = op {
                    op_crop_border(img, *border)
                } else {
                    Err(PilError::ValueError("expected CropBorder op".into()))
                }
            },
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
