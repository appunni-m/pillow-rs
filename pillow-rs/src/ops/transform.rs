//! Pillow-compatible geometric transforms and integer reduction.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, ResampleFilter, TransformMethod};

/// Host-neutral transform data after the binding has performed only Python
/// sequence extraction.
#[derive(Debug, Clone)]
pub enum TransformData {
    /// Six affine coefficients in Pillow order.
    Affine(Vec<f64>),
    /// `(bbox, quad)` mesh records before core validation and flattening.
    Mesh(Vec<(Vec<f64>, Vec<f64>)>),
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
    /// A value that could not be represented as an integer or sequence.
    Invalid,
}

/// Host-neutral optional crop input for the public `Image.reduce` method.
#[derive(Debug, Clone)]
pub enum ReduceBox {
    /// Crop coordinates in Pillow order `(left, upper, right, lower)`.
    Sequence(Vec<i64>),
    /// A value that could not be represented as a coordinate sequence.
    Invalid,
}

fn transform_component(value: i64) -> Result<u8, PilError> {
    u8::try_from(value).map_err(|_| PilError::ValueError("bytes must be in range(0, 256)".into()))
}

fn flatten_mesh_data(items: &[(Vec<f64>, Vec<f64>)]) -> Result<Vec<f64>, PilError> {
    let mut flat = Vec::with_capacity(items.len().saturating_mul(12));
    for (bbox, quad) in items {
        if bbox.len() != 4 {
            return Err(PilError::ValueError(
                "mesh_flatten: each bbox must have exactly 4 values [x0, y0, x1, y1]".into(),
            ));
        }
        if quad.len() != 8 {
            return Err(PilError::ValueError(
                "mesh_flatten: each quad must have exactly 8 values [x0, y0, …, x3, y3]".into(),
            ));
        }
        flat.extend_from_slice(bbox);
        flat.extend_from_slice(quad);
    }
    Ok(flat)
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
            ReduceFactor::Invalid => {
                return Err(PilError::TypeError(
                    "factor must be an integer or a sequence of two integers".into(),
                ));
            }
        };
        if factors.len() != 2 {
            return Err(PilError::TypeError(format!(
                "argument 1 must be sequence of length 2, not {}",
                factors.len()
            )));
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
                            .map_err(|_| PilError::ValueError("box coordinate overflow".into()))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.crop(Some((coords[0], coords[1], coords[2], coords[3])))?
            }
            ReduceBox::Sequence(values) => {
                return Err(PilError::TypeError(format!(
                    "box must be a 4-item sequence, not {}",
                    values.len()
                )));
            }
            ReduceBox::Invalid => {
                return Err(PilError::TypeError("box must be a 4-item sequence".into()));
            }
        };
        source.reduce(x_factor, y_factor)
    }

    fn public_transform_fill(
        &self,
        fillcolor: Option<TransformFill>,
    ) -> Result<((u8, u8, u8, u8), Option<u8>), PilError> {
        let mode = self.mode()?;
        let default_fill = if mode == "CMYK" {
            (0, 0, 0, 0)
        } else {
            (0, 0, 0, 255)
        };
        let palette_mode = self.has_palette_mode();
        let default_palette_fill = palette_mode.then_some(0);
        match fillcolor {
            None => Ok((default_fill, default_palette_fill)),
            Some(TransformFill::Scalar(value)) if palette_mode => {
                let index = value.clamp(0, i64::from(u8::MAX)) as u8;
                Ok(((index, 0, 0, 255), Some(index)))
            }
            Some(TransformFill::Scalar(value)) => {
                let value = transform_component(value)?;
                Ok(((value, value, value, 255), None))
            }
            Some(TransformFill::Components(values)) => {
                if !matches!(values.len(), 3 | 4) {
                    return Err(PilError::ValueError(
                        "color must be int, or tuple of one, three or four elements".into(),
                    ));
                }
                let r = transform_component(values[0])?;
                let g = transform_component(values[1])?;
                let b = transform_component(values[2])?;
                let a = if values.len() == 4 {
                    transform_component(values[3])?
                } else {
                    255
                };
                if palette_mode && a != 255 {
                    return Err(PilError::ValueError(
                        "cannot add non-opaque RGBA color to RGB palette".into(),
                    ));
                }
                Ok(((r, g, b, a), default_palette_fill))
            }
            Some(TransformFill::Name(name)) => {
                // Color parsing is deliberately performed in core even though
                // Pillow's transform path later resolves named fills through a
                // temporary palette and uses index zero for the raster fill.
                crate::color::parse_color_str(&name)?;
                Ok((default_fill, default_palette_fill))
            }
            Some(TransformFill::Invalid) => {
                Err(PilError::TypeError("color must be int or tuple".into()))
            }
        }
    }

    /// Applies a Pillow public transform method after host values have been
    /// converted to neutral Rust inputs.
    ///
    /// Method `0` is affine and method `4` is mesh. Resampling and fill are
    /// retained in the public signature for compatibility; the current core
    /// transform backend samples with nearest-neighbor, matching the existing
    /// implementation.
    pub fn transform_public(
        &self,
        size: (u32, u32),
        method: i32,
        data: Option<TransformData>,
        _resample: i32,
        _fill: i32,
        fillcolor: Option<TransformFill>,
    ) -> Result<Image, PilError> {
        let (fill, palette_fill) = self.public_transform_fill(fillcolor)?;
        match method {
            0 => {
                let Some(TransformData::Affine(matrix)) = data else {
                    return Err(PilError::ValueError("missing method data".into()));
                };
                self.transform_affine_with_palette_fill(size, &matrix, fill, palette_fill)
            }
            4 => {
                let Some(TransformData::Mesh(items)) = data else {
                    return Err(PilError::ValueError("MESH requires data".into()));
                };
                let data = flatten_mesh_data(&items)?;
                self.transform_mesh(size, data, fill)
            }
            _ => Err(PilError::NotImplementedError(format!(
                "Transform method '{method}' not yet implemented"
            ))),
        }
    }

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
