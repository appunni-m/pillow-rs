//! Imageops operation registration.
use super::prelude::{PilError, Registry};
fn register_autocontrast_equalize(m: &mut Registry) {
    m.insert(
        "Autocontrast",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::ops::prepare_autocontrast_pipeline,
            "autocontrast.wgsl"
        ),
    );

    m.insert(
        "Equalize",
        gpu_entry!(
            crate::compute::pool_cpu::ops::imageops::ops::prepare_equalize_pipeline,
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
            crate::compute::pool_cpu::ops::geometry::ops::prepare_pipeline,
            "contain.wgsl"
        ),
    );

    m.insert(
        "Cover",
        gpu_entry!(
            crate::compute::pool_cpu::ops::geometry::ops::prepare_pipeline,
            "cover.wgsl"
        ),
    );
}

fn register_fit_pad(m: &mut Registry) {
    m.insert(
        "Fit",
        gpu_entry!(
            crate::compute::pool_cpu::ops::geometry::ops::prepare_pipeline,
            "fit.wgsl"
        ),
    );

    m.insert(
        "Pad",
        gpu_entry!(
            crate::compute::pool_cpu::ops::geometry::ops::prepare_pipeline,
            "pad.wgsl"
        ),
    );
}

fn register_scale_expand(m: &mut Registry) {
    m.insert(
        "Scale",
        gpu_entry!(
            crate::compute::pool_cpu::ops::geometry::ops::prepare_pipeline,
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
