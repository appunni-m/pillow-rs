use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

impl Image {
    /// Crops using `(x, y, width, height)` coordinates.
    ///
    /// Binding crates use this form after converting Pillow box coordinates.
    /// Use [`Image::crop_box`] when you already have `(left, top, right,
    /// bottom)`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when width or height is zero.
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

    /// Crops using Pillow box coordinates.
    ///
    /// `right` and `bottom` are exclusive edges. The method rejects boxes whose
    /// saturated width or height is zero.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the box has zero width or height.
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
