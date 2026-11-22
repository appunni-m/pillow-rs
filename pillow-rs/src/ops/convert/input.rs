use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::DitherMethod;
use crate::raster::Mode;

pub(super) fn parse_dither(s: Option<&str>) -> Result<Option<DitherMethod>, PilError> {
    match s {
        Some("NONE") | Some("none") => Ok(Some(DitherMethod::None)),
        Some("FLOYDSTEINBERG") | Some("floydsteinberg") => Ok(Some(DitherMethod::FloydSteinberg)),
        None => Ok(Some(DitherMethod::FloydSteinberg)), // PIL default: FloydSteinberg dither
        // Pillow-compatible conversion accepts the dither as an integer enum,
        // even though this internal parser also handles symbolic names.
        _ => Err(PilError::TypeError(
            "'str' object cannot be interpreted as an integer".into(),
        )),
    }
}

/// Host-neutral dither argument in Pillow's conversion contract.
#[derive(Debug, Clone)]
pub enum DitherInput {
    /// No dither argument was supplied.
    None,
    /// Integer enum; zero disables dithering and non-zero selects
    /// Floyd-Steinberg.
    Integer(u32),
    /// Signed C-int enum supplied by Python's compatibility wrapper.
    SignedInteger(i32),
    /// String value, which Pillow rejects for this entry point.
    Name(String),
    /// A value of another host type. The type name is used in Pillow's
    /// compatibility diagnostic.
    Invalid(String),
}

/// Validates a dither string against Pillow's integer-enum contract.
///
/// Pillow-compatible conversion accepts the dither enum as an integer, while
/// the lower-level Rust conversion API uses symbolic strings internally. Keep
/// this distinction in the shared core contract so each binding reports the
/// same error for a string argument.
///
/// # Errors
///
/// Always returns [`PilError::TypeError`] because strings are not valid dither
/// enum arguments in the Pillow-compatible conversion API.
pub fn validate_convert_dither(value: &str) -> Result<(), PilError> {
    let _ = parse_dither(Some(value));
    Err(PilError::TypeError(
        "'str' object cannot be interpreted as an integer".into(),
    ))
}

/// Normalizes the host-extracted dither value into the symbolic core form.
// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
pub fn normalize_convert_dither(value: DitherInput) -> Result<Option<String>, PilError> {
    match value {
        DitherInput::None => Ok(None),
        DitherInput::Integer(value) => Ok(Some(if value == 0 {
            "NONE".to_owned()
        } else {
            "FLOYDSTEINBERG".to_owned()
        })),
        DitherInput::SignedInteger(value) => Ok(Some(if value == 0 {
            "NONE".to_owned()
        } else {
            "FLOYDSTEINBERG".to_owned()
        })),
        DitherInput::Name(value) => validate_convert_dither(&value).map(|_| None),
        DitherInput::Invalid(type_name) => Err(PilError::TypeError(format!(
            "'{type_name}' object cannot be interpreted as an integer"
        ))),
    }
}

/// Host-neutral destination-mode argument for Pillow-compatible conversion.
#[derive(Debug, Clone)]
pub enum ConvertModeInput {
    /// No destination mode was supplied.
    None,
    /// A destination mode parsed by the host binding.
    Mode(Mode),
    /// A string that is not a Pillow mode, retained only for Pillow's error
    /// ordering and message at the external compatibility boundary.
    InvalidName(String),
    /// A value of another host type.
    InvalidType(String),
}

/// Host-neutral palette argument for Pillow-compatible conversion.
#[derive(Debug, Clone)]
pub enum ConvertPaletteInput {
    /// No palette object was supplied.
    None,
    /// A symbolic palette name.
    Name(String),
    /// A value of another host type.
    Invalid(String),
}

/// Python truthiness checks Pillow performs while selecting a Convert route.
#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub enum ConvertMatrixTruthiness {
    /// The matrix argument's truth value has not been requested yet.
    NotEvaluated,
    /// One truthiness check has run.
    First(bool),
    /// Pillow checked the matrix in the same-mode fast path and again in the
    /// matrix branch. Preserve both outcomes for custom Python objects.
    Repeated { first: bool, second: bool },
}

/// Core-owned Convert branch selection used to stage host argument extraction.
#[derive(Debug, Clone, Copy)]
#[doc(hidden)]
pub enum ConvertInputPlan {
    /// The host adapter must evaluate `bool(matrix)` before planning continues.
    NeedsMatrixTruthiness,
    /// Same-mode conversion checked a truthy matrix twice in Pillow's wrapper.
    NeedsMatrixTruthinessAgain { first: bool },
    /// The host adapter must compare `palette` with `Palette.ADAPTIVE`.
    NeedsPaletteComparison,
    /// Return a mode-preserving copy without reading optional arguments.
    Copy,
    /// Execute a matrix conversion for this target mode.
    Matrix { target_mode: Mode },
    /// Quantize the source to P, consuming only the color count.
    Quantize { target_mode: Mode },
    /// Execute an ordinary mode conversion, consuming only dither.
    Convert { target_mode: Mode },
}

fn copy_with_context(image: &Image, context: Option<crate::compute::ExecutionContext>) -> Image {
    match context {
        Some(context) => image.clone().with_execution_context(context).copy(),
        None => image.copy(),
    }
}

/// Pillow-compatible image mode conversion methods.
impl Image {
    /// Applies Pillow's default-mode, palette, and matrix validation before
    /// entering the shared conversion implementation.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn convert_with_input(
        &self,
        mode: ConvertModeInput,
        matrix: Option<Vec<f64>>,
        dither: DitherInput,
        palette: ConvertPaletteInput,
        colors: Option<u32>,
    ) -> Result<Image, PilError> {
        self.convert_with_input_and_context(mode, matrix, dither, palette, colors, None)
    }

    /// Selects Convert's core route before the host adapter converts optional
    /// arguments. `None` in `palette_adaptive` requests the comparison only on
    /// the route where Pillow evaluates it.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_convert_input_plan(
        &self,
        mode: ConvertModeInput,
        matrix_truthiness: ConvertMatrixTruthiness,
        palette_adaptive: Option<bool>,
    ) -> Result<ConvertInputPlan, PilError> {
        let source_mode = self.mode_identity()?;
        if matches!(
            &mode,
            ConvertModeInput::InvalidName(_) | ConvertModeInput::InvalidType(_)
        ) {
            let Some(matrix_truthy) = first_matrix_truthiness(matrix_truthiness) else {
                return Ok(ConvertInputPlan::NeedsMatrixTruthiness);
            };
            if matrix_truthy {
                return Err(PilError::ValueError("illegal conversion".to_owned()));
            }
            return Err(invalid_convert_mode_error(mode));
        }
        let target_mode = match mode {
            ConvertModeInput::None if source_mode != Mode::ImagePalette8 => {
                return Ok(ConvertInputPlan::Copy);
            }
            ConvertModeInput::None => {
                let default_mode = self
                    .palette_mode()
                    .and_then(Mode::from_pillow_name)
                    .unwrap_or(Mode::ImageRgb8);
                if default_mode == Mode::ImageRgb8 && self.has_transparency_data() {
                    Mode::ImageRgba8
                } else {
                    default_mode
                }
            }
            ConvertModeInput::Mode(target) => target,
            ConvertModeInput::InvalidName(_) | ConvertModeInput::InvalidType(_) => {
                return Err(PilError::InternalError(
                    "invalid Convert mode passed the mode preflight".into(),
                ));
            }
        };

        if target_mode == source_mode {
            match matrix_truthiness {
                ConvertMatrixTruthiness::NotEvaluated => {
                    return Ok(ConvertInputPlan::NeedsMatrixTruthiness);
                }
                ConvertMatrixTruthiness::First(false) => return Ok(ConvertInputPlan::Copy),
                ConvertMatrixTruthiness::First(true) => {
                    return Ok(ConvertInputPlan::NeedsMatrixTruthinessAgain { first: true });
                }
                ConvertMatrixTruthiness::Repeated { second: false, .. } => {
                    return Ok(ConvertInputPlan::Convert { target_mode });
                }
                ConvertMatrixTruthiness::Repeated { second: true, .. } => {}
            }
        }

        let matrix_truthy = match matrix_truthiness {
            ConvertMatrixTruthiness::NotEvaluated => {
                return Ok(ConvertInputPlan::NeedsMatrixTruthiness);
            }
            ConvertMatrixTruthiness::First(value) => value,
            ConvertMatrixTruthiness::Repeated { first, .. } => first,
        };
        if matrix_truthy {
            // NO_PILLOW_SUPPORT: Rust also permits direct matrix conversion to LAB.
            if matches!(
                target_mode,
                Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImageLab8
            ) {
                return Ok(ConvertInputPlan::Matrix { target_mode });
            }
            return Err(PilError::ValueError("illegal conversion".to_owned()));
        }
        if target_mode == source_mode {
            return Ok(ConvertInputPlan::Copy);
        }
        if target_mode == Mode::ImagePalette8 && source_mode == Mode::ImageRgba8 {
            return Ok(ConvertInputPlan::Quantize { target_mode });
        }
        if target_mode == Mode::ImagePalette8 {
            return match palette_adaptive {
                None => Ok(ConvertInputPlan::NeedsPaletteComparison),
                Some(true) => Ok(ConvertInputPlan::Quantize { target_mode }),
                Some(false) => Ok(ConvertInputPlan::Convert { target_mode }),
            };
        }
        Ok(ConvertInputPlan::Convert { target_mode })
    }

    /// Executes a finalized Convert plan after the host extracts only values
    /// consumed by the selected branch.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn execute_convert_input_plan(
        &self,
        plan: ConvertInputPlan,
        matrix: Option<Vec<f64>>,
        dither: DitherInput,
        colors: Option<i64>,
        context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Image, PilError> {
        match plan {
            ConvertInputPlan::Copy => Ok(copy_with_context(self, context)),
            ConvertInputPlan::Matrix { target_mode } => {
                let matrix = matrix.ok_or_else(|| {
                    PilError::InternalError("Convert matrix plan has no matrix input".into())
                })?;
                validate_matrix_length(&matrix)?;
                match context {
                    Some(context) => self.convert_with_execution_context(
                        target_mode,
                        Some(matrix),
                        None,
                        context,
                    ),
                    None => self.convert(target_mode, Some(matrix), None, None, None),
                }
            }
            ConvertInputPlan::Quantize { target_mode } => {
                if target_mode != Mode::ImagePalette8 {
                    return Err(PilError::InternalError(
                        "Convert quantize plan has a non-P target".into(),
                    ));
                }
                // TODO(pillow-rs#3ptb): preserve Image.convert's transparency
                // preflight, palette remap, and metadata order on adaptive-P routes.
                let colors = colors.unwrap_or(256);
                let colors = i32::try_from(colors).map_err(|_| {
                    PilError::OverflowError("Python int too large to convert to C int".into())
                })?;
                self.quantize_with_visible_mode_and_context(
                    Some(colors),
                    None,
                    None,
                    crate::ops::quantize::QuantizePalette::None,
                    None,
                    None,
                    context,
                )
            }
            ConvertInputPlan::Convert { target_mode } => {
                let dither = normalize_convert_dither(dither)?;
                match context {
                    Some(context) => self.convert_with_execution_context(
                        target_mode,
                        None,
                        dither.as_deref(),
                        context,
                    ),
                    None => self.convert(target_mode, None, dither.as_deref(), None, None),
                }
            }
            ConvertInputPlan::NeedsMatrixTruthiness
            | ConvertInputPlan::NeedsMatrixTruthinessAgain { .. }
            | ConvertInputPlan::NeedsPaletteComparison => Err(PilError::InternalError(
                "Convert input plan was executed before host staging completed".into(),
            )),
        }
    }

    /// Applies Pillow's wrapper conversion contract under an injected route.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn convert_with_input_and_context(
        &self,
        mode: ConvertModeInput,
        matrix: Option<Vec<f64>>,
        dither: DitherInput,
        palette: ConvertPaletteInput,
        colors: Option<u32>,
        context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Image, PilError> {
        let matrix_truthiness = match matrix.is_some() {
            true => ConvertMatrixTruthiness::Repeated {
                first: true,
                second: true,
            },
            false => ConvertMatrixTruthiness::First(false),
        };
        let palette_adaptive = Some(matches!(
            palette,
            ConvertPaletteInput::Name(name) if name == "ADAPTIVE"
        ));
        let plan = self.prepare_convert_input_plan(mode, matrix_truthiness, palette_adaptive)?;
        self.execute_convert_input_plan(plan, matrix, dither, colors.map(i64::from), context)
    }
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn first_matrix_truthiness(value: ConvertMatrixTruthiness) -> Option<bool> {
    match value {
        ConvertMatrixTruthiness::NotEvaluated => None,
        ConvertMatrixTruthiness::First(value) => Some(value),
        ConvertMatrixTruthiness::Repeated { first, .. } => Some(first),
    }
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn invalid_convert_mode_error(mode: ConvertModeInput) -> PilError {
    match mode {
        ConvertModeInput::InvalidName(_) => PilError::ValueError("image has wrong mode".into()),
        ConvertModeInput::InvalidType(type_name) => {
            PilError::TypeError(format!("argument 1 must be str, not {type_name}"))
        }
        ConvertModeInput::None | ConvertModeInput::Mode(_) => PilError::InternalError(
            "valid Convert mode was sent to invalid-mode error handling".into(),
        ),
    }
}

// VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=n/a
fn validate_matrix_length(matrix: &[f64]) -> Result<(), PilError> {
    if matches!(matrix.len(), 4 | 12) {
        Ok(())
    } else {
        Err(PilError::TypeError(
            "matrix must be tuple of length 4 or 12".to_owned(),
        ))
    }
}
