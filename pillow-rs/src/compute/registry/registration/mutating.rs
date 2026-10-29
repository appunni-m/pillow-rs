//! Mutating operation registration.
use super::prelude::{PilError, Registry};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "PutPixel",
        gpu_entry!(
            crate::compute::pool_cpu::ops::put::ops::prepare_pixel_pipeline,
            "put_pixel.wgsl"
        ),
    );

    m.insert(
        "PutData",
        gpu_entry!(
            crate::compute::pool_cpu::ops::put::ops::prepare_data_pipeline,
            "put_data.wgsl"
        ),
    );

    m.insert(
        "Color3DLut",
        gpu_entry!(
            crate::compute::pool_cpu::ops::color3dlut::ops::prepare_pipeline,
            "color_3dlut.wgsl"
        ),
    );
}

fn register_part_2(m: &mut Registry) {
    m.insert(
        "PutAlpha",
        gpu_entry!(
            crate::compute::pool_cpu::ops::put::ops::prepare_alpha_pipeline,
            "put_alpha.wgsl"
        ),
    );

    m.insert(
        "PutAlphaData",
        gpu_entry!(
            crate::compute::pool_cpu::ops::put::ops::prepare_alpha_data_pipeline,
            "put_alpha_data.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    register_part_2(m);
    Ok(())
}
