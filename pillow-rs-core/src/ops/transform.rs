//! Image.transform — affine, perspective, and mesh transforms.
//! Also Image.reduce for box-downscaling.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};

impl Image {
    /// Apply an affine transform: `[a, b, c, d, e, f]` where
    /// x' = a*x + b*y + c,  y' = d*x + e*y + f
    pub fn transform_affine(
        &self,
        size: (u32, u32),
        matrix: &[f64],
        fillcolor: (u8, u8, u8, u8),
    ) -> Result<Image, PilError> {
        if matrix.len() != 6 {
            return Err(PilError::ValueError(
                "Affine transform requires 6 coefficients [a,b,c,d,e,f]".into(),
            ));
        }
        let (dst_w, dst_h) = size;
        let data = matrix.to_vec();
        // Build a 3x3 affine matrix padded to 9 values for TransformMethod::Affine
        let fill = Some(fillcolor);
        Ok(Image::push_op(
            self,
            PipelineOp::Transform {
                w: dst_w,
                h: dst_h,
                method: TransformMethod::Affine,
                data,
                filter: ResampleFilter::Bilinear,
                fill,
            },
        ))
    }

    /// Reduce image size by an integer factor (box downsampling).
    pub fn reduce(&self, factor: u32) -> Result<Image, PilError> {
        Ok(Image::push_op(self, PipelineOp::Reduce { factor }))
    }
}
