//! Pillow-compatible geometric transforms and integer reduction.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};

mod fill;
mod public;

/// Host-neutral transform data after the binding has performed only Python
/// sequence extraction.
#[derive(Debug, Clone)]
pub enum TransformData {
    /// Six affine coefficients in Pillow order.
    Affine(Vec<f64>),
    /// `(bbox, quad)` mesh records before core validation and flattening.
    Mesh(Vec<(Vec<f64>, Vec<f64>)>),
    /// Nested mesh records whose item arity still belongs to core validation.
    RawMesh(Vec<Vec<Vec<f64>>>),
    /// A mapping supplied where Pillow expects mesh data. Pillow iterates the
    /// mapping's keys and reports the resulting unpacking error from core.
    Mapping,
    /// A string supplied where Pillow expects mesh data. Each character is a
    /// one-item iterable, so the core reports the unpacking error.
    Text(String),
    /// A value that could not be interpreted as affine or mesh data.
    Invalid(String),
}

/// Host-neutral fill-color input for public `Image.transform` calls.
#[derive(Debug, Clone)]
pub enum TransformFill {
    /// A scalar Pillow fill value.
    Scalar(i64),
    /// A three- or four-component fill value.
    Components(Vec<i64>),
    /// A named/hex color that must be parsed by core.
    Name(String),
    /// A sequence whose components are not integers.
    FloatingComponents(Vec<f64>),
    /// A scalar real value, accepted by mode F.
    FloatingScalar(f64),
    /// An object that is not a supported fill representation.
    Invalid,
}

/// Host-neutral factor input for the public `Image.reduce` method.
#[derive(Debug, Clone)]
pub enum ReduceFactor {
    /// A single factor applied to both axes.
    Scalar(i64),
    /// A factor sequence supplied by the caller.
    Sequence(Vec<i64>),
    /// A factor sequence whose items are not integers.
    FloatingSequence(Vec<f64>),
    /// A value that could not be represented as an integer or sequence.
    Invalid(String),
}

/// Host-neutral optional crop input for the public `Image.reduce` method.
#[derive(Debug, Clone)]
pub enum ReduceBox {
    /// Crop coordinates in Pillow order `(left, upper, right, lower)`.
    Sequence(Vec<i64>),
    /// A value that could not be represented as a coordinate sequence.
    Invalid,
    /// A value with a known host type that is not a coordinate sequence.
    InvalidType(String),
}

impl Image {
    /// Reduces an image using Pillow's public scalar/sequence contract.
    ///
    /// Python and other bindings only extract primitive values into the
    /// neutral enums above. Factor arity, positivity, coordinate conversion,
    /// and the optional crop all remain in the core implementation.
    pub fn reduce_public(
        &self,
        factor: ReduceFactor,
        box_coords: Option<ReduceBox>,
    ) -> Result<Image, PilError> {
        let factors = match factor {
            ReduceFactor::Scalar(value) => [value, value].to_vec(),
            ReduceFactor::Sequence(values) => values,
            ReduceFactor::FloatingSequence(_) => {
                return Err(PilError::TypeError(
                    "'float' object cannot be interpreted as an integer".into(),
                ));
            }
            ReduceFactor::Invalid(type_name) => {
                return Err(PilError::TypeError(format!(
                    "'{type_name}' object cannot be interpreted as an integer"
                )));
            }
        };
        if factors.len() != 2 {
            return Err(PilError::TypeError(format!(
                "argument 1 must be sequence of length 2, not {}",
                factors.len()
            )));
        }
        if self.has_palette_mode() && (box_coords.is_some() || factors.as_slice() != [1, 1]) {
            // Pillow permits the indexed-image no-op P.reduce(1), but
            // ImagingReduce rejects actual indexed reduction and every
            // optional-box form before constructing a result image.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        let x_factor = u32::try_from(factors[0])
            .ok()
            .filter(|&value| value > 0)
            .ok_or_else(|| PilError::ValueError("scale must be > 0".into()))?;
        let y_factor = u32::try_from(factors[1])
            .ok()
            .filter(|&value| value > 0)
            .ok_or_else(|| PilError::ValueError("scale must be > 0".into()))?;

        let Some(box_coords) = box_coords else {
            return self.reduce(x_factor, y_factor);
        };
        let source = match box_coords {
            ReduceBox::Sequence(values) if values.len() == 4 => {
                let coords = values
                    .into_iter()
                    .map(|value| {
                        i32::try_from(value)
                            // Pillow converts the Python coordinate through a C
                            // long before entering ImagingReduce; an out-of-range
                            // value is therefore an OverflowError, not a generic
                            // value-shape failure.
                            .map_err(|_| {
                                if value > i64::from(i32::MAX) {
                                    PilError::OverflowError(
                                        "signed integer is greater than maximum".into(),
                                    )
                                } else {
                                    PilError::OverflowError(
                                        "signed integer is less than minimum".into(),
                                    )
                                }
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                // Pillow's imaging core rejects reversed reduce boxes before
                // crop validation and reports the generic empty-box error.
                if coords[2] < coords[0] || coords[3] < coords[1] {
                    return Err(PilError::ValueError("box can't be empty".into()));
                }
                self.crop(Some((coords[0], coords[1], coords[2], coords[3])))?
            }
            ReduceBox::Sequence(values) => {
                return Err(PilError::TypeError(format!(
                    "argument 2 must be sequence of length 4, not {}",
                    values.len()
                )));
            }
            ReduceBox::Invalid => {
                return Err(PilError::TypeError("box must be a 4-item sequence".into()));
            }
            ReduceBox::InvalidType(type_name) => {
                return Err(PilError::TypeError(format!(
                    "argument 2 must be 4-item sequence, not {type_name}"
                )));
            }
        };
        source.reduce(x_factor, y_factor)
    }

    fn transform_affine_palette_index_with_filter(
        &self,
        size: (u32, u32),
        matrix: &[f64],
        fill_index: u8,
        filter: ResampleFilter,
        fill_is_none: bool,
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
            filter,
            fill_is_none,
        )
    }

    fn transform_affine_with_palette_fill(
        &self,
        size: (u32, u32),
        matrix: &[f64],
        fillcolor: (u8, u8, u8, u8),
        palette_fill: Option<u8>,
        filter: ResampleFilter,
        fill_is_none: bool,
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
                data: data.into(),
                filter,
                fill,
                fill_is_none,
                palette_fill,
            },
        ))
    }

    fn transform_perspective_with_palette_fill(
        &self,
        size: (u32, u32),
        data: &[f64],
        fillcolor: (u8, u8, u8, u8),
        palette_fill: Option<u8>,
        filter: ResampleFilter,
        fill_is_none: bool,
    ) -> Result<Image, PilError> {
        if data.len() != 8 {
            return Err(PilError::ValueError("wrong number of data points".into()));
        }
        Ok(Image::push_op(
            self,
            PipelineOp::Transform {
                w: size.0,
                h: size.1,
                method: TransformMethod::Perspective,
                data: data.to_vec().into(),
                filter,
                fill: Some(fillcolor),
                fill_is_none,
                palette_fill,
            },
        ))
    }

    fn transform_quad_with_palette_fill(
        &self,
        size: (u32, u32),
        data: &[f64],
        fillcolor: (u8, u8, u8, u8),
        palette_fill: Option<u8>,
        filter: ResampleFilter,
        fill_is_none: bool,
    ) -> Result<Image, PilError> {
        if data.len() != 8 {
            return Err(PilError::ValueError("wrong number of data points".into()));
        }
        Ok(Image::push_op(
            self,
            PipelineOp::Transform {
                w: size.0,
                h: size.1,
                method: TransformMethod::Quad,
                data: data.to_vec().into(),
                filter,
                fill: Some(fillcolor),
                fill_is_none,
                palette_fill,
            },
        ))
    }

    /// Reduces image size by integer factors using box downsampling.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; invalid factor handling is reported by
    /// pipeline execution.
    pub fn reduce(&self, x_factor: u32, y_factor: u32) -> Result<Image, PilError> {
        Ok(Image::push_op(
            self,
            PipelineOp::Reduce { x_factor, y_factor },
        ))
    }

    fn transform_mesh_with_filter(
        &self,
        size: (u32, u32),
        data: Vec<f64>,
        fillcolor: (u8, u8, u8, u8),
        filter: ResampleFilter,
        fill_is_none: bool,
    ) -> Result<Image, PilError> {
        Ok(Image::push_op(
            self,
            PipelineOp::Transform {
                w: size.0,
                h: size.1,
                method: TransformMethod::Mesh,
                data: data.into(),
                filter,
                fill: Some(fillcolor),
                fill_is_none,
                palette_fill: None,
            },
        ))
    }
}
