use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Image {
    /// Crop expects (x, y, width, height) — the Python wrapper converts
    /// Pillow's (left, top, right, bottom) to this format internally.
    pub fn crop(&self, box_coords: (u32, u32, u32, u32)) -> Result<Image, PilError> {
        let (x, y, w, h) = box_coords;
        if w == 0 || h == 0 {
            return Err(PilError::ValueError(
                "crop box must have positive dimensions".into(),
            ));
        }
        Ok(Image::push_op(
            self,
            PipelineOp::Crop {
                left: x,
                top: y,
                right: x + w,
                bottom: y + h,
            },
        ))
    }

    /// Crop using PIL box format: (left, top, right, bottom).
    /// Computes width = right - left, height = bottom - top internally.
    pub fn crop_box(
        &self,
        left: u32,
        top: u32,
        right: u32,
        bottom: u32,
    ) -> Result<Image, PilError> {
        let w = right.saturating_sub(left);
        let h = bottom.saturating_sub(top);
        if w == 0 || h == 0 {
            return Err(PilError::ValueError(
                "crop box must have positive dimensions".into(),
            ));
        }
        Ok(Image::push_op(
            self,
            PipelineOp::Crop {
                left,
                top,
                right,
                bottom,
            },
        ))
    }
}
