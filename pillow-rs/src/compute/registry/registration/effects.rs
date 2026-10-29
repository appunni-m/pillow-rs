//! Effects operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, op_alpha_composite, op_paste,
};
fn register_part_1(m: &mut Registry) {
    // Pillow 12.2.0 libImaging/Effects.c:117-159 consumes process-global
    // rand() sequentially and performs collision-prone scatter writes. The GPU
    // entry receives a host-generated relocation map, so its gather shader
    // preserves that order-dependent contract without inventing a second RNG.
    m.insert(
        "EffectSpread",
        gpu_entry!(
            crate::compute::pool_cpu::ops::effects::ops::prepare_spread_pipeline,
            "effect_spread.wgsl"
        ),
    );

    m.insert(
        "Paste",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::Paste {
                    source,
                    x,
                    y,
                    w: _,
                    h: _,
                    mask,
                    mask_alpha,
                } = op
                {
                    op_paste(img, source, *x as i64, *y as i64, mask, *mask_alpha, _mode)
                } else {
                    Err(PilError::ValueError("expected Paste op".into()))
                }
            },
            "paste.wgsl"
        ),
    );

    m.insert(
        "AlphaComposite",
        gpu_entry!(
            |img, op, _mode| {
                if let PipelineOp::AlphaComposite { source, .. } = op {
                    op_alpha_composite(img, source)
                } else {
                    Err(PilError::ValueError("expected AlphaComposite op".into()))
                }
            },
            "alpha_composite.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    Ok(())
}
