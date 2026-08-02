use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{PipelineOp, TransposeMethod};

/// Host-neutral input for Pillow's integer-or-name transpose argument.
#[derive(Debug, Clone)]
pub enum TransposeInput {
    /// Pillow's integer enum value.
    Index(i64),
    /// A symbolic transpose name.
    Name(String),
    /// A value of another host type.
    Invalid(String),
}

/// Returns Pillow's symbolic transpose name for an integer enum value.
pub fn transpose_name_from_int(value: i64) -> &'static str {
    match value {
        0 => "FLIP_LEFT_RIGHT",
        1 => "FLIP_TOP_BOTTOM",
        2 => "ROTATE_90",
        3 => "ROTATE_180",
        4 => "ROTATE_270",
        5 => "TRANSPOSE",
        6 => "TRANSVERSE",
        _ => "FLIP_LEFT_RIGHT",
    }
}

/// Normalize the integer-or-name orientation accepted by `TransposedFont`.
///
/// The Python facade stores the public value unchanged; this helper owns the
/// shared enum-to-name mapping used by the font and image paths.
pub fn normalize_transpose_input(input: TransposeInput) -> Result<String, PilError> {
    match input {
        TransposeInput::Index(index) => Ok(transpose_name_from_int(index).to_owned()),
        TransposeInput::Name(name) => Ok(name),
        TransposeInput::Invalid(type_name) => Err(PilError::TypeError(format!(
            "'{type_name}' object cannot be interpreted as an integer"
        ))),
    }
}

/// Normalize the orientation stored by `ImageFont.TransposedFont`.
///
/// Pillow defers errors for an object used as the orientation until a font
/// operation needs to interpret it. Preserve that constructor timing by
/// representing an unconvertible host value as an absent normalized name.
pub fn normalize_transposed_font_input(input: TransposeInput) -> Option<String> {
    normalize_transpose_input(input).ok()
}

impl Image {
    /// Applies the Python-facing integer/name transpose contract.
    pub fn transpose_with_input(&self, input: TransposeInput) -> Result<Image, PilError> {
        let method = normalize_transpose_input(input)?;
        self.transpose(&method)
    }

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
