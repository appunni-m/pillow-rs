use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, TransposeMethod};

impl Image {
    /// Applies a Pillow transpose method.
    ///
    /// Accepted methods are `"FLIP_LEFT_RIGHT"`, `"FLIP_TOP_BOTTOM"`,
    /// `"ROTATE_90"`, `"ROTATE_180"`, `"ROTATE_270"`, `"TRANSPOSE"`, and
    /// `"TRANSVERSE"`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `method` is unknown.
    pub fn transpose(&self, method: &str) -> Result<Image, PilError> {
        let method = parse_transpose(method)?;
        Ok(Image::push_op(self, PipelineOp::Transpose { method }))
    }
}

fn parse_transpose(method: &str) -> Result<TransposeMethod, PilError> {
    match method {
        "FLIP_LEFT_RIGHT" => Ok(TransposeMethod::FlipLeftRight),
        "FLIP_TOP_BOTTOM" => Ok(TransposeMethod::FlipTopBottom),
        "ROTATE_90" => Ok(TransposeMethod::Rotate90),
        "ROTATE_180" => Ok(TransposeMethod::Rotate180),
        "ROTATE_270" => Ok(TransposeMethod::Rotate270),
        "TRANSPOSE" => Ok(TransposeMethod::Transpose),
        "TRANSVERSE" => Ok(TransposeMethod::Transverse),
        _ => Err(PilError::ValueError(format!(
            "Unknown transpose method: {}. Use FLIP_LEFT_RIGHT, FLIP_TOP_BOTTOM, ROTATE_90, ROTATE_180, ROTATE_270, TRANSPOSE, or TRANSVERSE.",
            method
        ))),
    }
}
