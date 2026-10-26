//! Enhance operation registration.
use super::prelude::{
    DynamicImage, PilError, PipelineOp, Registry, op_enhance_brightness,
    op_enhance_color_saturation, op_enhance_contrast, op_enhance_sharpness,
};
fn register_brightness_contrast(m: &mut Registry) {
    m.insert(
        "Brightness",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Brightness { factor } = op {
                    op_enhance_brightness(img, *factor, mode)
                } else {
                    Err(PilError::ValueError("expected Brightness op".into()))
                }
            },
            "brightness.wgsl"
        ),
    );

    m.insert(
        "Contrast",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Contrast { factor } = op {
                    op_enhance_contrast(img, *factor, mode)
                } else {
                    Err(PilError::ValueError("expected Contrast op".into()))
                }
            },
            "contrast.wgsl"
        ),
    );
}

fn register_color_sharpness(m: &mut Registry) {
    m.insert(
        "ColorSaturation",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::ColorSaturation { factor } = op {
                    op_enhance_color_saturation(img, *factor, mode)
                } else {
                    Err(PilError::ValueError("expected ColorSaturation op".into()))
                }
            },
            "color_saturation.wgsl"
        ),
    );

    m.insert(
        "Sharpness",
        gpu_entry!(
            |img: &DynamicImage,
             op: &PipelineOp,
             mode: Option<&str>|
             -> Result<DynamicImage, PilError> {
                if let PipelineOp::Sharpness { factor } = op {
                    op_enhance_sharpness(img, *factor, mode)
                } else {
                    Err(PilError::ValueError("expected Sharpness op".into()))
                }
            },
            "sharpness.wgsl"
        ),
    );
}

pub(super) fn register(m: &mut Registry) -> Result<(), PilError> {
    register_brightness_contrast(m);
    register_color_sharpness(m);
    Ok(())
}
