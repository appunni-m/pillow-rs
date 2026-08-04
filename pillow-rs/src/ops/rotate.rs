use crate::error::PilError;
use crate::image::Image;
use crate::ops::imageops::ImageOpsColor;
use crate::pipeline::PipelineOp;

/// Host-neutral resampling input for Pillow's rotate wrapper.
#[derive(Debug, Clone)]
pub enum RotateResampleInput {
    /// No explicit resampling value was supplied.
    None,
    /// A numeric Pillow resampling code.
    Code(i64),
    /// A non-string value that is not an integer or string resampling code.
    Other,
    /// A symbolic resampling name or an invalid string to validate.
    Name(String),
}

fn rotate_uses_nearest(input: &RotateResampleInput) -> bool {
    match input {
        RotateResampleInput::None | RotateResampleInput::Other => true,
        RotateResampleInput::Code(code) => *code == 0,
        RotateResampleInput::Name(name) => name == "NEAREST",
    }
}

fn unknown_resample_filter(value: impl std::fmt::Display) -> PilError {
    PilError::ValueError(format!(
        "Unknown resampling filter ({value}). Use Image.Resampling.NEAREST (0), Image.Resampling.BILINEAR (2) or Image.Resampling.BICUBIC (3)"
    ))
}

fn unsupported_rotate_resample(code: i64) -> PilError {
    let name = match code {
        1 => Some("LANCZOS"),
        4 => Some("BOX"),
        5 => Some("HAMMING"),
        _ => None,
    };
    match name {
        Some(name) => PilError::ValueError(format!(
            "Image.Resampling.{name} ({code}) cannot be used. Use Image.Resampling.NEAREST (0), Image.Resampling.BILINEAR (2) or Image.Resampling.BICUBIC (3)"
        )),
        None => unknown_resample_filter(code),
    }
}

/// Host-neutral boolean input for Pillow's rotate wrapper.
#[derive(Debug, Clone)]
pub enum RotateExpandInput {
    /// The explicit boolean value.
    Boolean(bool),
    /// An integer accepted by Pillow's truth-value conversion.
    Integer(i64),
}

/// Host-neutral center or translation input for Pillow's rotate wrapper.
#[derive(Debug, Clone)]
pub enum RotatePointInput {
    /// No point was supplied, or `None` was supplied.
    Default,
    /// A numeric sequence.
    Values(Vec<f64>),
    /// A non-subscriptable value classified at the binding boundary.
    Invalid {
        /// Python type name used in Pillow's diagnostic.
        type_name: String,
        /// Python truth value, used to preserve rotate's fast-path ordering.
        truthy: bool,
    },
}

impl RotatePointInput {
    fn is_truthy(&self) -> bool {
        match self {
            Self::Default => false,
            Self::Values(values) => !values.is_empty(),
            Self::Invalid { truthy, .. } => *truthy,
        }
    }

    fn validate(&self) -> Result<(), PilError> {
        match self {
            Self::Default => Ok(()),
            Self::Values(values) if values.len() >= 2 => Ok(()),
            Self::Values(_) => Err(PilError::IndexError("tuple index out of range".into())),
            Self::Invalid { type_name, .. } => Err(PilError::TypeError(format!(
                "'{type_name}' object is not subscriptable"
            ))),
        }
    }

    fn into_values(self) -> Option<(f64, f64)> {
        match self {
            Self::Values(values) => values.first().copied().zip(values.get(1).copied()),
            _ => None,
        }
    }
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
        RotateExpandInput::Boolean(value) => value,
        RotateExpandInput::Integer(value) => value != 0,
    };
    if angle % 360.0 != 0.0 {
        match resample {
            RotateResampleInput::None => return Err(unknown_resample_filter("None")),
            RotateResampleInput::Code(code) if !matches!(code, 0 | 2 | 3) => {
                return Err(unsupported_rotate_resample(code));
            }
            RotateResampleInput::Name(value)
                if !matches!(value.as_str(), "NEAREST" | "BILINEAR" | "BICUBIC") =>
            {
                return Err(unknown_resample_filter(value));
            }
            RotateResampleInput::Other => {}
            _ => {}
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
        center: RotatePointInput,
        translate: RotatePointInput,
        fillcolor: ImageOpsColor,
    ) -> Result<Image, PilError> {
        let normalized_angle = angle % 360.0;
        let nearest = rotate_uses_nearest(&resample);
        // Pillow skips resampling-name validation only for an exact multiple of
        // 360 degrees. Route every other angle through the public normalizer;
        // its contract is specifically the non-zero-angle path.
        let expand = if normalized_angle % 360.0 != 0.0 {
            normalize_python_rotate(resample, expand)?
        } else {
            normalize_python_rotate_at_angle(normalized_angle, resample, expand)?
        };
        let fillcolor = crate::ops::imageops::resolve_imageops_color(fillcolor, &self.mode()?)?;
        let center_truthy = center.is_truthy();
        let translate_truthy = translate.is_truthy();
        if center_truthy || translate_truthy {
            center.validate()?;
            translate.validate()?;
        }
        let center = center.into_values();
        let translate = translate.into_values();
        self.rotate_with_options(
            normalized_angle,
            expand,
            fillcolor,
            center,
            translate,
            nearest,
        )
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
        fillcolor: Option<(u8, u8, u8, u8)>,
    ) -> Result<Image, PilError> {
        self.rotate_with_options(angle, expand, fillcolor, None, None, true)
    }

    fn rotate_with_options(
        &self,
        angle: f64,
        expand: bool,
        fillcolor: Option<(u8, u8, u8, u8)>,
        center: Option<(f64, f64)>,
        translate: Option<(f64, f64)>,
        nearest: bool,
    ) -> Result<Image, PilError> {
        let angle = angle % 360.0;
        Ok(Image::push_op(
            self,
            PipelineOp::Rotate {
                angle,
                expand,
                fill: fillcolor,
                center,
                translate,
                nearest,
            },
        ))
    }
}
