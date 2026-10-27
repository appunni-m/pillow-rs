use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::DitherMethod;

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
pub fn normalize_convert_dither(value: DitherInput) -> Result<Option<String>, PilError> {
    match value {
        DitherInput::None => Ok(None),
        DitherInput::Integer(value) => Ok(Some(if value == 0 {
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
    /// A destination mode name.
    Name(String),
    /// A value of another host type.
    Invalid(String),
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

/// Pillow-compatible image mode conversion methods.
impl Image {
    /// Applies Pillow's default-mode, palette, and matrix validation before
    /// entering the shared conversion implementation.
    pub fn convert_with_input(
        &self,
        mode: ConvertModeInput,
        matrix: Option<Vec<f64>>,
        dither: DitherInput,
        palette: ConvertPaletteInput,
        colors: Option<u32>,
    ) -> Result<Image, PilError> {
        let source_mode = self.mode()?;
        let target_mode = match mode {
            ConvertModeInput::None => {
                if source_mode != "P" {
                    return Ok(self.copy());
                }
                let mut target = self.palette_mode().unwrap_or("RGB").to_owned();
                if target == "RGB" && self.has_transparency_data() {
                    target = "RGBA".to_owned();
                }
                target
            }
            ConvertModeInput::Name(target) => target,
            ConvertModeInput::Invalid(type_name) => {
                return Err(PilError::TypeError(format!(
                    "argument 1 must be str, not {type_name}"
                )));
            }
        };

        if target_mode == source_mode && matrix.is_none() {
            return Ok(self.copy());
        }
        // Pillow rejects matrix conversions to unsupported target modes before
        // it validates the coefficient count. This ordering is observable for
        // an invalid mode paired with a short matrix.
        if matrix.is_some() && !matches!(target_mode.as_str(), "L" | "RGB") {
            return Err(PilError::ValueError("illegal conversion".to_owned()));
        }
        if let Some(matrix) = matrix.as_ref()
            && !matches!(matrix.len(), 4 | 12)
        {
            return Err(PilError::TypeError(format!(
                "argument 2 must be sequence of length 12, not {}",
                matrix.len()
            )));
        }

        let palette = match palette {
            ConvertPaletteInput::None => None,
            ConvertPaletteInput::Name(name) => Some(name),
            // Pillow accepts and ignores palette values for the public
            // conversion paths exercised here; only the target mode and
            // dither/matrix inputs affect conversion dispatch.
            ConvertPaletteInput::Invalid(_) => None,
        };
        // Keep Pillow's dither coercion in the Rust core. Pillow ignores the
        // argument on a same-mode no-op and on matrix conversions, and reports
        // matrix/mode errors before it attempts to interpret the dither enum.
        let dither = if matrix.is_some() {
            None
        } else {
            normalize_convert_dither(dither)?
        };
        self.convert(
            &target_mode,
            matrix,
            dither.as_deref(),
            palette.as_deref(),
            colors,
        )
    }
}
