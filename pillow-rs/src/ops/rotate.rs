use crate::error::PilError;
use crate::image::Image;
use crate::ops::imageops::ImageOpsColor;
use crate::pipeline::{PipelineOp, ResampleFilter};
use crate::raster::Mode;

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
        /// Host type name used in Pillow's diagnostic.
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

/// A parsed and validated Rotate request ready to enqueue on an image.
///
/// The Python adapter uses this between argument validation and dirty-palette
/// realization so validation happens once before the source captures the new
/// palette metadata in its output graph.
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct PreparedRotate {
    mode: Option<Mode>,
    angle: f64,
    expand: bool,
    fill: Option<(u8, u8, u8, u8)>,
    center: Option<(f64, f64)>,
    translate: Option<(f64, f64)>,
    filter: ResampleFilter,
    nearest: bool,
}

/// Validates Pillow-compatible rotate arguments and returns the effective
/// expansion flag.
pub fn normalize_rotate(
    resample: RotateResampleInput,
    expand: RotateExpandInput,
) -> Result<bool, PilError> {
    normalize_rotate_at_angle(1.0, resample, expand)
}

fn normalize_rotate_at_angle(
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
    /// Parses and validates Rotate inputs without adding an operation.
    #[doc(hidden)]
    pub fn prepare_rotate_with_input(
        &self,
        angle: f64,
        resample: RotateResampleInput,
        expand: RotateExpandInput,
        center: RotatePointInput,
        translate: RotatePointInput,
        fillcolor: ImageOpsColor,
    ) -> Result<PreparedRotate, PilError> {
        let (normalized_angle, expand, requested_filter, requested_nearest) =
            normalize_rotate_request(angle, resample, expand)?;
        let source_mode = self.mode_identity()?;
        let nearest = requested_nearest
            || matches!(source_mode, Mode::One | Mode::ImagePalette8);
        let filter = if nearest {
            ResampleFilter::Nearest
        } else {
            requested_filter
        };
        let fill = resolve_rotate_fill(self, source_mode, fillcolor)?;
        let (center, translate) = resolve_rotate_points(center, translate)?;
        Ok(PreparedRotate {
            mode: Some(source_mode),
            angle: normalized_angle.rem_euclid(360.0),
            expand,
            fill,
            center,
            translate,
            filter,
            nearest,
        })
    }

    /// Queues a request returned by [`Self::prepare_rotate_with_input`].
    #[doc(hidden)]
    pub fn rotate_prepared(&self, prepared: PreparedRotate) -> Result<Image, PilError> {
        if let Some(prepared_mode) = prepared.mode {
            if self.mode_identity()? != prepared_mode {
                return Err(PilError::InternalError(
                    "prepared Rotate request belongs to a different image mode".into(),
                ));
            }
        }
        Ok(self.enqueue_rotate(prepared))
    }

    /// Applies the Pillow-compatible Rotate contract before queuing rotation.
    pub fn rotate_with_input(
        &self,
        angle: f64,
        resample: RotateResampleInput,
        expand: RotateExpandInput,
        center: RotatePointInput,
        translate: RotatePointInput,
        fillcolor: ImageOpsColor,
    ) -> Result<Image, PilError> {
        let prepared =
            self.prepare_rotate_with_input(angle, resample, expand, center, translate, fillcolor)?;
        Ok(self.enqueue_rotate(prepared))
    }

    fn enqueue_rotate(&self, prepared: PreparedRotate) -> Image {
        Image::push_op(
            self,
            PipelineOp::Rotate {
                angle: prepared.angle,
                expand: prepared.expand,
                fill: prepared.fill,
                center: prepared.center,
                translate: prepared.translate,
                filter: prepared.filter,
                nearest: prepared.nearest,
            },
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
        let prepared = PreparedRotate {
            mode: None,
            angle: angle.rem_euclid(360.0),
            expand,
            fill: fillcolor,
            center,
            translate,
            filter,
            nearest,
        };
        Ok(self.enqueue_rotate(prepared))
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
}

fn normalize_rotate_request(
    angle: f64,
    resample: RotateResampleInput,
    expand: RotateExpandInput,
) -> Result<(f64, bool, ResampleFilter, bool), PilError> {
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
    // 360 degrees. Route every other angle through the public normalizer.
    let expand = if normalized_angle % 360.0 != 0.0 {
        normalize_rotate(resample, expand)?
    } else {
        normalize_rotate_at_angle(normalized_angle, resample, expand)?
    };
    Ok((normalized_angle, expand, requested_filter, requested_nearest))
}

fn resolve_rotate_fill(
    image: &Image,
    source_mode: Mode,
    fillcolor: ImageOpsColor,
) -> Result<Option<(u8, u8, u8, u8)>, PilError> {
    match fillcolor {
        ImageOpsColor::None => Ok(None),
        color => {
            use crate::ops::transform::TransformFill;
            let fill = match color {
                ImageOpsColor::Scalar(value) => TransformFill::Scalar(value),
                ImageOpsColor::Components(values) => TransformFill::Components(values),
                ImageOpsColor::TupleComponents(values) => TransformFill::Components(values),
                ImageOpsColor::TupleNumericComponents(_) => TransformFill::Invalid,
                ImageOpsColor::TupleComponentsWithFallback { .. } => TransformFill::Invalid,
                ImageOpsColor::WideTupleComponents { .. } => TransformFill::Invalid,
                ImageOpsColor::ListComponents(_)
                | ImageOpsColor::ListNumericComponents(_)
                | ImageOpsColor::InvalidList =>
                {
                    TransformFill::InvalidList
                }
                ImageOpsColor::FloatingScalar(value) => TransformFill::FloatingScalar(value),
                ImageOpsColor::FloatingComponents(values) => {
                    TransformFill::FloatingComponents(values)
                }
                ImageOpsColor::Name(name) => TransformFill::Name(name),
                ImageOpsColor::Invalid => TransformFill::Invalid,
                ImageOpsColor::None => unreachable!(),
            };
            let (fill, _) = crate::ops::transform::public_transform_fill(Some(fill), source_mode)?;
            // Two-sample luma-alpha carriers read their alpha fill from byte 3.
            Ok(Some(if matches!(
                source_mode,
                Mode::ImageLumaA8
                    | Mode::ImageLaPremultiplied8
                    | Mode::ImagePaletteAlpha8
                    | Mode::ImageLumaA16
            ) {
                (fill.0, fill.0, fill.0, fill.1)
            } else {
                fill
            }))
        }
    }
}

fn resolve_rotate_points(
    center: RotatePointInput,
    translate: RotatePointInput,
) -> Result<(Option<(f64, f64)>, Option<(f64, f64)>), PilError> {
    if center.is_truthy() || translate.is_truthy() {
        center.validate()?;
        translate.validate()?;
    }
    Ok((center.into_values(), translate.into_values()))
}
