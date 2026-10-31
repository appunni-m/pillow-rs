//! Enhancement operation registration.
use super::prelude::Registry;
fn register_brightness_contrast(m: &mut Registry) {
    m.insert(
        "Brightness",
        gpu_entry!(
            crate::compute::pool_cpu::ops::enhance::ops::prepare_pipeline,
            "brightness.wgsl"
        ),
    );

    m.insert(
        "Contrast",
        gpu_entry!(
            crate::compute::pool_cpu::ops::enhance::ops::prepare_contrast_pipeline,
            "contrast.wgsl"
        ),
    );
}

fn register_color_sharpness(m: &mut Registry) {
    m.insert(
        "ColorSaturation",
        gpu_entry!(
            crate::compute::pool_cpu::ops::enhance::ops::prepare_color_saturation_pipeline,
            "color_saturation.wgsl"
        ),
    );

    m.insert(
        "Sharpness",
        gpu_entry!(
            crate::compute::pool_cpu::ops::enhance::ops::prepare_sharpness_pipeline,
            "sharpness.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_brightness_contrast(m);
    register_color_sharpness(m);
    Ok(())
}
