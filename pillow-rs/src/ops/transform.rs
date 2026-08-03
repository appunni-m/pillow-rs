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
    /// Nested mesh records whose item arity still belongs to core validation.
    RawMesh(Vec<Vec<Vec<f64>>>),
    /// A mapping supplied where Pillow expects mesh data. Pillow iterates the
    /// mapping's keys and reports the resulting unpacking error from core.
    Mapping,
    /// A value that could not be interpreted as affine or mesh data.
    Invalid,
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
    /// A value with a known host type that is not a coordinate sequence.
    InvalidType(String),
}

fn transform_component(value: i64) -> Result<u8, PilError> {
    u8::try_from(value).map_err(|_| PilError::ValueError("bytes must be in range(0, 256)".into()))
}

fn clamp_transform_component(value: i64) -> u8 {
    value.clamp(0, i64::from(u8::MAX)) as u8
}

fn packed_transform_scalar(value: i64) -> (u8, u8, u8, u8) {
    let bytes = value.to_le_bytes();
    (bytes[0], bytes[1], bytes[2], bytes[3])
}

fn packed_transform_float(value: i64) -> (u8, u8, u8, u8) {
    let bytes = (value as f32).to_le_bytes();
    (bytes[0], bytes[1], bytes[2], bytes[3])
}

fn transform_fill_arity_error(mode: &str) -> PilError {
    let message = match mode {
        "1" | "L" | "P" | "I" | "I;16" | "I;16L" | "I;16B" | "I;16N" => {
            "color must be int or single-element tuple"
        }
        "F" => "must be real number, not tuple",
        "LA" | "PA" => "color must be int, or tuple of one or two elements",
        _ => "color must be int, or tuple of one, three or four elements",
    };
    PilError::TypeError(message.into())
}

fn transform_color_value_fill(
    mode: &str,
    color: crate::color::ColorValue,
) -> Result<(u8, u8, u8, u8), PilError> {
    let clamp = |value: i32| value.clamp(0, i32::from(u8::MAX)) as u8;
    match color {
        crate::color::ColorValue::Gray(value) => match mode {
            "F" => Ok(packed_transform_float(i64::from(value))),
            "I" => Ok(packed_transform_scalar(i64::from(value))),
            "I;16" | "I;16L" | "I;16B" | "I;16N" => {
                let bytes = (value as u16).to_le_bytes();
                Ok((bytes[0], bytes[1], 0, 0))
            }
            _ => Ok((clamp(value), 0, 0, 0)),
        },
        crate::color::ColorValue::GrayAlpha(gray, alpha) => Ok((clamp(gray), clamp(alpha), 0, 0)),
        crate::color::ColorValue::Rgb(red, green, blue) => Ok((
            clamp(red),
            clamp(green),
            clamp(blue),
            if matches!(mode, "RGBA" | "CMYK") {
                255
            } else {
                0
            },
        )),
        crate::color::ColorValue::Rgba(red, green, blue, alpha) => Ok((
            clamp(red),
            clamp(green),
            clamp(blue),
            if mode == "RGB" || mode == "YCbCr" || mode == "HSV" {
                0
            } else {
                clamp(alpha)
            },
        )),
        crate::color::ColorValue::Hsv(hue, saturation, value) => {
            Ok((clamp(hue), clamp(saturation), clamp(value), 0))
        }
    }
}

fn flatten_mesh_data(items: &[(Vec<f64>, Vec<f64>)]) -> Result<Vec<f64>, PilError> {
    let mut flat = Vec::with_capacity(items.len().saturating_mul(12));
    for (bbox, quad) in items {
        if bbox.len() != 4 {
            return Err(PilError::IndexError("tuple index out of range".into()));
        }
        if quad.len() != 8 {
            return Err(PilError::IndexError("tuple index out of range".into()));
        }
        flat.extend_from_slice(bbox);
        flat.extend_from_slice(quad);
    }
    Ok(flat)
}

fn flatten_raw_mesh_data(items: &[Vec<Vec<f64>>]) -> Result<Vec<f64>, PilError> {
    let mut flat = Vec::with_capacity(items.len().saturating_mul(12));
    for item in items {
        if item.len() != 2 {
            let message = if item.len() < 2 {
                format!(
                    "not enough values to unpack (expected 2, got {})",
                    item.len()
                )
            } else {
                "too many values to unpack (expected 2)".into()
            };
            return Err(PilError::ValueError(message));
        }
        let bbox = &item[0];
        let quad = &item[1];
        if bbox.len() != 4 {
            return Err(PilError::IndexError("tuple index out of range".into()));
        }
        if quad.len() != 8 {
            return Err(PilError::IndexError("tuple index out of range".into()));
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

    fn public_transform_fill(
        &self,
        fillcolor: Option<TransformFill>,
    ) -> Result<((u8, u8, u8, u8), Option<u8>), PilError> {
        let mode = self.mode()?;
        let palette_mode = self.has_palette_mode();
        let default_palette_fill = palette_mode.then_some(0);
        match fillcolor {
            // Image.transform creates an uninitialized image when no fill
            // color is supplied. The native transform then leaves exposed
            // pixels zeroed, including alpha, rather than using opaque black.
            None => Ok(((0, 0, 0, 0), default_palette_fill)),
            Some(TransformFill::Scalar(value)) if palette_mode => {
                let index = value.clamp(0, i64::from(u8::MAX)) as u8;
                Ok(((index, 0, 0, 255), Some(index)))
            }
            Some(TransformFill::Scalar(value)) => {
                let fill = match mode.as_str() {
                    "1" | "L" => (clamp_transform_component(value), 0, 0, 0),
                    "F" => packed_transform_float(value),
                    "I" => packed_transform_scalar(value),
                    "I;16" | "I;16L" | "I;16B" | "I;16N" => {
                        let bytes = (value as u16).to_le_bytes();
                        (bytes[0], bytes[1], 0, 0)
                    }
                    _ => packed_transform_scalar(value),
                };
                Ok((fill, None))
            }
            Some(TransformFill::Components(values)) => {
                let fill = match mode.as_str() {
                    "P" => match values.as_slice() {
                        [value] => {
                            let index = (*value).clamp(0, i64::from(u8::MAX)) as u8;
                            return Ok(((index, 0, 0, 255), Some(index)));
                        }
                        [red, green, blue] => {
                            let r = transform_component(*red)?;
                            let g = transform_component(*green)?;
                            let b = transform_component(*blue)?;
                            let _ = (r, g, b);
                            return Ok(((0, 0, 0, 255), default_palette_fill));
                        }
                        [red, green, blue, alpha] => {
                            let r = transform_component(*red)?;
                            let g = transform_component(*green)?;
                            let b = transform_component(*blue)?;
                            let a = transform_component(*alpha)?;
                            let _ = (r, g, b);
                            if a != 255 {
                                return Err(PilError::ValueError(
                                    "cannot add non-opaque RGBA color to RGB palette".into(),
                                ));
                            }
                            return Ok(((0, 0, 0, 255), default_palette_fill));
                        }
                        _ => return Err(transform_fill_arity_error(&mode)),
                    },
                    "1" | "L" => match values.as_slice() {
                        [value] => (clamp_transform_component(*value), 0, 0, 0),
                        _ => return Err(transform_fill_arity_error(&mode)),
                    },
                    "I" => match values.as_slice() {
                        [value] => packed_transform_scalar(*value),
                        _ => return Err(transform_fill_arity_error(&mode)),
                    },
                    "F" => match values.as_slice() {
                        [value] => packed_transform_float(*value),
                        _ => return Err(transform_fill_arity_error(&mode)),
                    },
                    "I;16" | "I;16L" | "I;16B" | "I;16N" => match values.as_slice() {
                        [value] => {
                            let bytes = (*value as u16).to_le_bytes();
                            (bytes[0], bytes[1], 0, 0)
                        }
                        _ => return Err(transform_fill_arity_error(&mode)),
                    },
                    "LA" | "PA" => match values.as_slice() {
                        [value] => (clamp_transform_component(*value), 0, 0, 0),
                        [gray, alpha] => (
                            clamp_transform_component(*gray),
                            clamp_transform_component(*alpha),
                            0,
                            0,
                        ),
                        _ => return Err(transform_fill_arity_error(&mode)),
                    },
                    _ if matches!(mode.as_str(), "RGB" | "YCbCr" | "HSV") => {
                        match values.as_slice() {
                            [value] => packed_transform_scalar(*value),
                            [red, green, blue] | [red, green, blue, _] => (
                                clamp_transform_component(*red),
                                clamp_transform_component(*green),
                                clamp_transform_component(*blue),
                                0,
                            ),
                            _ => return Err(transform_fill_arity_error(&mode)),
                        }
                    }
                    _ => match values.as_slice() {
                        [value] => packed_transform_scalar(*value),
                        [red, green, blue] => (
                            clamp_transform_component(*red),
                            clamp_transform_component(*green),
                            clamp_transform_component(*blue),
                            255,
                        ),
                        [red, green, blue, alpha] => (
                            clamp_transform_component(*red),
                            clamp_transform_component(*green),
                            clamp_transform_component(*blue),
                            clamp_transform_component(*alpha),
                        ),
                        _ => return Err(transform_fill_arity_error(&mode)),
                    },
                };
                Ok((fill, default_palette_fill))
            }
            Some(TransformFill::Name(name)) => {
                // Color parsing and mode conversion belong to core. Pillow's
                // palette transform resolves named colors through a temporary
                // palette and rasterizes them as index zero.
                let (r, g, b, a) = crate::color::parse_color_str_unclamped(&name)?;
                if palette_mode {
                    crate::color::getcolor(r, g, b, a, &mode)?;
                    return Ok(((0, 0, 0, 255), default_palette_fill));
                }
                let color = crate::color::getcolor(r, g, b, a, &mode)?;
                Ok((transform_color_value_fill(&mode, color)?, None))
            }
            Some(TransformFill::FloatingComponents(_)) => Err(PilError::TypeError(
                "'float' object cannot be interpreted as an integer".into(),
            )),
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
            1 => {
                let Some(TransformData::Affine(extent)) = data else {
                    return Err(PilError::ValueError("missing method data".into()));
                };
                let [x0, y0, x1, y1] = extent.as_slice() else {
                    return Err(PilError::ValueError(
                        "extent transform needs 4 coordinates".into(),
                    ));
                };
                // Pillow's EXTENT method maps each destination pixel back
                // into the requested source rectangle. Express that mapping
                // through the same affine pipeline used by method 0 so mode,
                // fill, and lazy execution remain shared.
                let matrix = vec![
                    (x1 - x0) / f64::from(size.0),
                    0.0,
                    *x0,
                    0.0,
                    (y1 - y0) / f64::from(size.1),
                    *y0,
                ];
                self.transform_affine_with_palette_fill(size, &matrix, fill, palette_fill)
            }
            4 => {
                let data = match data {
                    Some(TransformData::Mesh(items)) => flatten_mesh_data(&items)?,
                    Some(TransformData::RawMesh(items)) => flatten_raw_mesh_data(&items)?,
                    Some(TransformData::Affine(items)) if items.is_empty() => {
                        let mode = self.mode()?;
                        if let Some(index) = palette_fill {
                            return Ok(Image::new_palette_index(size.0, size.1, index));
                        }
                        return Image::new(size.0, size.1, &mode, fill);
                    }
                    Some(TransformData::Affine(_)) => {
                        return Err(PilError::TypeError(
                            "cannot unpack non-iterable int object".into(),
                        ));
                    }
                    Some(TransformData::Mapping) => {
                        return Err(PilError::ValueError(
                            "too many values to unpack (expected 2)".into(),
                        ));
                    }
                    Some(TransformData::Invalid) => {
                        return Err(PilError::TypeError(
                            "transform data must be a sequence".into(),
                        ));
                    }
                    None => return Err(PilError::ValueError("missing method data".into())),
                };
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
