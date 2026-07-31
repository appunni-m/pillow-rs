//! Pillow-compatible geometric transforms and integer reduction.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};

impl Image {
    /// Applies an affine transform and returns a lazy result image.
    ///
    /// `matrix` must contain `[a, b, c, d, e, f]`, where
    /// `x' = a*x + b*y + c` and `y' = d*x + e*y + f`.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when `matrix` does not contain exactly
    /// six coefficients.
    pub fn transform_affine(
        &self,
        size: (u32, u32),
        matrix: &[f64],
        fillcolor: (u8, u8, u8, u8),
    ) -> Result<Image, PilError> {
        let palette_fill = self.has_palette_mode().then_some(0);
        self.transform_affine_with_palette_fill(size, matrix, fillcolor, palette_fill)
    }

    /// Applies an affine transform to a `P` image using a raw fill index.
    ///
    /// Pillow preserves a scalar `fillcolor` as a palette index, while tuple
    /// and string colors resolve to index zero. This entry point retains that
    /// distinction after binding argument conversion.
    ///
    /// # Errors
    ///
    /// Returns [`PilError::ValueError`] when the source is not mode `P`, or
    /// when `matrix` does not contain exactly six coefficients.
    pub fn transform_affine_palette_index(
        &self,
        size: (u32, u32),
        matrix: &[f64],
        fill_index: u8,
    ) -> Result<Image, PilError> {
        if !self.has_palette_mode() {
            return Err(PilError::ValueError(
                "palette fill index requires mode P".into(),
            ));
        }
        self.transform_affine_with_palette_fill(
            size,
            matrix,
            (fill_index, 0, 0, 255),
            Some(fill_index),
        )
    }

    fn transform_affine_with_palette_fill(
        &self,
        size: (u32, u32),
        matrix: &[f64],
        fillcolor: (u8, u8, u8, u8),
        palette_fill: Option<u8>,
    ) -> Result<Image, PilError> {
        if matrix.len() != 6 {
            return Err(PilError::ValueError(
                "wrong number of matrix entries".into(),
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
                filter: ResampleFilter::Nearest,
                fill,
                palette_fill,
            },
        ))
    }

    /// Reduces image size by an integer factor using box downsampling.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; invalid factor handling is reported by
    /// pipeline execution.
    pub fn reduce(&self, factor: u32) -> Result<Image, PilError> {
        Ok(Image::push_op(self, PipelineOp::Reduce { factor }))
    }

    /// Applies a mesh transform using piecewise quadrilateral mappings.
    ///
    /// `data` carries the transform coefficients expected by the pipeline
    /// backend for Pillow-style mesh transforms.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; malformed mesh data is reported by
    /// pipeline execution.
    pub fn transform_mesh(
        &self,
        size: (u32, u32),
        data: Vec<f64>,
        fillcolor: (u8, u8, u8, u8),
    ) -> Result<Image, PilError> {
        Ok(Image::push_op(
            self,
            PipelineOp::Transform {
                w: size.0,
                h: size.1,
                method: TransformMethod::Mesh,
                data,
                filter: ResampleFilter::Nearest,
                fill: Some(fillcolor),
                palette_fill: None,
            },
        ))
    }
}
