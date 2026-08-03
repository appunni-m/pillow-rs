use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

/// Host-neutral resampling input for Pillow's rotate wrapper.
#[derive(Debug, Clone)]
pub enum RotateResampleInput {
    /// No explicit resampling value was supplied.
    None,
    /// A non-string value. Pillow's rotate wrapper currently ignores this
    /// value after checking that it is not a string.
    Other,
    /// A symbolic resampling name or an invalid string to validate.
    Name(String),
}

/// Host-neutral boolean input for Pillow's rotate wrapper.
#[derive(Debug, Clone)]
pub enum RotateExpandInput {
    /// The explicit boolean value.
    Boolean(bool),
    /// A non-boolean value.
    Invalid,
}

/// Validates the Python-facing rotate arguments and returns the effective
/// expansion flag.
pub fn normalize_python_rotate(
    resample: RotateResampleInput,
    expand: RotateExpandInput,
) -> Result<bool, PilError> {
    normalize_python_rotate_at_angle(1.0, resample, expand)
}

fn normalize_python_rotate_at_angle(
    angle: f64,
    resample: RotateResampleInput,
    expand: RotateExpandInput,
) -> Result<bool, PilError> {
    let expand = match expand {
        RotateExpandInput::Boolean(value) => Ok(value),
        RotateExpandInput::Invalid => Err(PilError::TypeError(
            "'int' object is not subscriptable".to_owned(),
        )),
    }?;
    if angle % 360.0 != 0.0 {
        if let RotateResampleInput::Name(value) = resample {
            if !matches!(value.as_str(), "NEAREST" | "BILINEAR" | "BICUBIC") {
                return Err(PilError::ValueError(format!(
                    "Unknown resampling filter ({value}). Use Image.Resampling.NEAREST (0), Image.Resampling.BILINEAR (2) or Image.Resampling.BICUBIC (3)"
                )));
            }
        }
    }
    Ok(expand)
}

impl Image {
    /// Applies the Python-facing rotate contract before queuing rotation.
    pub fn rotate_with_input(
        &self,
        angle: f64,
        resample: RotateResampleInput,
        expand: RotateExpandInput,
        fillcolor: Option<(u8, u8, u8, u8)>,
    ) -> Result<Image, PilError> {
        let normalized_angle = angle % 360.0;
        let expand = normalize_python_rotate_at_angle(normalized_angle, resample, expand)?;
        self.rotate(normalized_angle, expand, fillcolor)
    }

    /// Rotates the image by `angle` degrees.
    ///
    /// When `expand` is true, the output canvas expands to contain the rotated
    /// image. `fillcolor` is used for newly exposed pixels. The angle is
    /// normalized into Pillow's `0..360` degree range.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn rotate(
        &self,
        angle: f64,
        expand: bool,
        _fillcolor: Option<(u8, u8, u8, u8)>,
    ) -> Result<Image, PilError> {
        let angle = angle % 360.0;
        Ok(Image::push_op(
            self,
            PipelineOp::Rotate {
                angle,
                expand,
                fill: _fillcolor,
            },
        ))
    }
}
