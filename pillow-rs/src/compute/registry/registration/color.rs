//! Color operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, op_convert, op_convert_lab, op_remap_palette,
};
fn register_part_1(m: &mut Registry) {
    m.insert(
        "Convert",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             source_mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Convert {
                    mode: cm,
                    matrix: _,
                    dither: _,
                } = op
                {
                    op_convert(img, cm, source_mode)
                } else {
                    Err(PilError::ValueError("expected Convert op".into()))
                }
            },
            "convert.wgsl"
        ),
    );

    m.insert(
        "ConvertLab",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             _mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if matches!(op, PipelineOp::ConvertLab { .. }) {
                    op_convert_lab(img)
                } else {
                    Err(PilError::ValueError("expected ConvertLab op".into()))
                }
            },
            "rgb_to_lab.wgsl"
        ),
    );

    m.insert(
        "RemapPalette",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::RemapPalette { dest_map } = op {
                    // Pass dest_map as-is (capped at 256 per PIL behavior).
                    // op_remap_palette handles short dest_maps correctly.
                    let end = dest_map.len().min(256);
                    op_remap_palette(img, &dest_map[..end], mode)
                } else {
                    Err(PilError::ValueError("expected RemapPalette op".into()))
                }
            },
            "remap_palette.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_part_1(m);
    Ok(())
}
