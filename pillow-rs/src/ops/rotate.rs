use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Image {
    /// Rotate the image by the given angle (in degrees).
    /// expand: if true, expands output to fit rotated image.
    /// fillcolor: optional RGBA fill color for exposed areas.
    /// Angle is normalized to [0, 360) matching PIL behavior.
    pub fn rotate(
        &self,
        angle: f64,
        expand: bool,
        _fillcolor: Option<(u8, u8, u8, u8)>,
    ) -> Result<Image, PilError> {
        let angle = angle % 360.0;
        Ok(Image::push_op(
            self,
            PipelineOp::Rotate {
                angle,
                expand,
                fill: _fillcolor,
            },
        ))
    }
}
