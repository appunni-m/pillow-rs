//! Pillow-compatible geometric transforms and integer reduction.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};

mod fill;
mod public;
pub(crate) use fill::public_transform_fill;

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
    /// A Python list, which Pillow does not unwrap as a color tuple.
    InvalidList,
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
    /// A factor sequence with a different arity, before item conversion.
    SequenceLength { length: usize, tuple: bool },
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
    /// A box sequence with a different arity, before item conversion.
    SequenceLength { length: usize, tuple: bool },
    /// A value that could not be represented as a coordinate sequence.
    Invalid,
    /// A value with a known host type that is not a coordinate sequence.
    InvalidType(String),
}

fn reduce_c_int(value: i64) -> Result<i32, PilError> {
    i32::try_from(value).map_err(|_| {
        PilError::OverflowError(if value < 0 {
            "signed integer is less than minimum".into()
        } else {
            "signed integer is greater than maximum".into()
        })
    })
}

fn parse_reduce_factors(factor: ReduceFactor) -> Result<(i32, i32), PilError> {
    let values = match factor {
        ReduceFactor::Scalar(value) => [value, value].to_vec(),
        ReduceFactor::Sequence(values) => values,
        ReduceFactor::SequenceLength { length, tuple } => {
            return Err(PilError::TypeError(format!(
                "argument 1 must be {} of length 2, not {length}",
                if tuple { "tuple" } else { "sequence" }
            )));
        }
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
    if values.len() != 2 {
        return Err(PilError::TypeError(format!(
            "argument 1 must be sequence of length 2, not {}",
            values.len()
        )));
    }

    Ok((reduce_c_int(values[0])?, reduce_c_int(values[1])?))
}

fn validate_reduce_factors(x_factor: i32, y_factor: i32) -> Result<(u32, u32), PilError> {
    if x_factor <= 0 || y_factor <= 0 {
        return Err(PilError::ValueError("scale must be > 0".into()));
    }
    Ok((x_factor as u32, y_factor as u32))
}

fn parse_reduce_box_coordinates(
    box_coords: Option<ReduceBox>,
    width: i32,
    height: i32,
) -> Result<(i32, i32, i32, i32), PilError> {
    match box_coords {
        None => (0, 0, width, height),
        Some(ReduceBox::Sequence(values)) if values.len() == 4 => {
            let mut coordinates = [0; 4];
            for (slot, value) in coordinates.iter_mut().zip(values) {
                *slot = reduce_c_int(value)?;
            }
            (coordinates[0], coordinates[1], coordinates[2], coordinates[3])
        }
        Some(ReduceBox::Sequence(values)) => {
            return Err(PilError::TypeError(format!(
                "argument 2 must be sequence of length 4, not {}",
                values.len()
            )));
        }
        Some(ReduceBox::SequenceLength { length, tuple }) => {
            return Err(PilError::TypeError(format!(
                "argument 2 must be {} of length 4, not {length}",
                if tuple { "tuple" } else { "sequence" }
            )));
        }
        Some(ReduceBox::Invalid) => {
            return Err(PilError::TypeError("box must be a 4-item sequence".into()));
        }
        Some(ReduceBox::InvalidType(type_name)) => {
            return Err(PilError::TypeError(format!(
                "argument 2 must be 4-item tuple, not {type_name}"
            )));
        }
    }
}

fn validate_reduce_box(
    coordinates: (i32, i32, i32, i32),
    width: i32,
    height: i32,
) -> Result<(), PilError> {
    let (left, top, right, bottom) = coordinates;
    if left < 0 || top < 0 {
        return Err(PilError::ValueError("box offset can't be negative".into()));
    }
    if right > width || bottom > height {
        return Err(PilError::ValueError(
            "box can't exceed original image size".into(),
        ));
    }
    if right <= left || bottom <= top {
        return Err(PilError::ValueError("box can't be empty".into()));
    }
    Ok(())
}

impl Image {
    /// Reduces an image using Pillow's public scalar/sequence contract.
    ///
    /// Bindings preserve sequence arity and convert integer items through their
    /// host protocol. The core owns scale positivity, box bounds, mode
    /// rejection, and the optional crop.
    pub fn reduce_public(
        &self,
        factor: ReduceFactor,
        box_coords: Option<ReduceBox>,
    ) -> Result<Image, PilError> {
        let (x_scale, y_scale) = parse_reduce_factors(factor)?;
        let box_was_supplied = box_coords.is_some();
        let validated_factors = if box_was_supplied {
            None
        } else {
            let factors = validate_reduce_factors(x_scale, y_scale)?;
            if factors == (1, 1) {
                return Ok(self.copy());
            }
            Some(factors)
        };
        let (width, height) = self.size()?;
        let width = reduce_c_int(i64::from(width))?;
        let height = reduce_c_int(i64::from(height))?;
        let coordinates = parse_reduce_box_coordinates(box_coords, width, height)?;
        let (x_factor, y_factor) = match validated_factors {
            Some(factors) => factors,
            None => validate_reduce_factors(x_scale, y_scale)?,
        };
        validate_reduce_box(coordinates, width, height)?;

        if self.has_palette_mode() && (x_factor != 1 || y_factor != 1) {
            // Pillow validates scale and box bounds before ImagingReduce
            // rejects real P-mode downsampling.
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        if x_factor == 1 && y_factor == 1 {
            return self.crop(Some(coordinates));
        }

        let full_box = coordinates == (0, 0, width, height);
        if full_box {
            self.reduce(x_factor, y_factor)
        } else {
            self.crop(Some(coordinates))?.reduce(x_factor, y_factor)
        }
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
