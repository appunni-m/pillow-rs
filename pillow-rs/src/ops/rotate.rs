use crate::error::PilError;
use crate::image::Image;
use crate::ops::imageops::ImageOpsColor;
use crate::pipeline::{PipelineOp, ResampleFilter};

/// Host-neutral resampling input for Pillow's rotate wrapper.
#[derive(Debug, Clone)]
pub enum RotateResampleInput {
    /// No explicit resampling value was supplied.
    None,
    /// A numeric Pillow resampling code.
    Code(i64),
    /// A symbolic resampling name or an invalid string to validate.
    Name(String),
}

fn rotate_uses_nearest(input: &RotateResampleInput) -> bool {
    match input {
        RotateResampleInput::None => true,
        RotateResampleInput::Code(code) => *code == 0,
        RotateResampleInput::Name(name) => name == "NEAREST",
    }
}

fn rotate_filter(input: &RotateResampleInput) -> ResampleFilter {
    match input {
        RotateResampleInput::Code(2) => ResampleFilter::Bilinear,
        RotateResampleInput::Name(name) if name == "BILINEAR" => ResampleFilter::Bilinear,
        RotateResampleInput::Code(3) => ResampleFilter::Bicubic,
        RotateResampleInput::Name(name) if name == "BICUBIC" => ResampleFilter::Bicubic,
        _ => ResampleFilter::Nearest,
    }
}

/// Round a rotation coefficient with the same decimal contract as Pillow's
/// `round(value, 15)`. Formatting at fixed precision avoids the double-round
/// error of multiplying by `1e15` before a binary floating-point round (for
/// example, `sin(-45°)` must remain `-0.707106781186547`).
pub(crate) fn round_rotate_coefficient(value: f64) -> f64 {
    format!("{value:.15}").parse::<f64>().unwrap_or(value)
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
        let expand_requested = match &expand {
            RotateExpandInput::Boolean(value) => *value,
            RotateExpandInput::Integer(value) => *value != 0,
        };
        // Pillow calculates the expanded bounds with ``ceil``/``floor``.
        // Python raises while converting the NaN result when a non-finite
        // angle is used with expansion; keep the failure at rotate() rather
        // than queueing a pipeline that later reports a zero-sized image.
        if expand_requested && !angle.is_finite() {
            return Err(PilError::ValueError(
                "cannot convert float NaN to integer".into(),
            ));
        }
        let normalized_angle = angle % 360.0;
        let requested_nearest = rotate_uses_nearest(&resample);
        let requested_filter = rotate_filter(&resample);
        // Pillow skips resampling-name validation only for an exact multiple of
        // 360 degrees. Route every other angle through the public normalizer;
        // its contract is specifically the non-zero-angle path.
        let expand = if normalized_angle % 360.0 != 0.0 {
            normalize_python_rotate(resample, expand)?
        } else {
            normalize_python_rotate_at_angle(normalized_angle, resample, expand)?
        };
        // Pillow forces nearest-neighbour sampling for indexed images even
        // when the caller supplies BILINEAR/BICUBIC. Treat that effective
        // choice as part of the queued operation so the fast path and lazy
        // geometry planner see the same contract as the native rotate call.
        let source_mode = self.mode()?;
        let nearest = requested_nearest || matches!(source_mode.as_str(), "1" | "P");
        let filter = if nearest {
            ResampleFilter::Nearest
        } else {
            requested_filter
        };
        let fillcolor = crate::ops::imageops::resolve_imageops_color(fillcolor, &source_mode)?;
        let center_truthy = center.is_truthy();
        let translate_truthy = translate.is_truthy();
        if center_truthy || translate_truthy {
            center.validate()?;
            translate.validate()?;
        }
        let center = center.into_values();
        let translate = translate.into_values();
        // The ordinary nearest-neighbor call has the same normalized contract
        // as the public core method. Reuse it after Python-facing validation so
        // the Python and WASM entry points share one rotation constructor.
        if nearest && center.is_none() && translate.is_none() {
            return self.rotate_with_options(
                normalized_angle,
                expand,
                fillcolor,
                None,
                None,
                filter,
                true,
            );
        }
        self.rotate_with_options(
            normalized_angle,
            expand,
            fillcolor,
            center,
            translate,
            filter,
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
        self.rotate_with_options(
            angle,
            expand,
            fillcolor,
            None,
            None,
            ResampleFilter::Nearest,
            true,
        )
    }

    fn rotate_with_options(
        &self,
        angle: f64,
        expand: bool,
        fillcolor: Option<(u8, u8, u8, u8)>,
        center: Option<(f64, f64)>,
        translate: Option<(f64, f64)>,
        filter: ResampleFilter,
        nearest: bool,
    ) -> Result<Image, PilError> {
        // Python's ``angle % 360.0`` always yields a non-negative remainder.
        // Keep the normalized angle in the queued operation so trigonometric
        // rounding is independent of whether the caller supplied `-x` or
        // its equivalent `360-x`.
        let angle = angle.rem_euclid(360.0);
        Ok(Image::push_op(
            self,
            PipelineOp::Rotate {
                angle,
                expand,
                fill: fillcolor,
                center,
                translate,
                filter,
                nearest,
            },
        ))
    }
}
