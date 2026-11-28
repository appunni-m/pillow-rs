use crate::error::PilError;
use crate::image::{
    Image, ImageInfoValue, ImagingCoreTransparencyKey, PaletteColorComponent, PaletteModel,
};
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
// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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

/// Typed value of Pillow's wrapper-visible Image.info["transparency"] key.
#[derive(Debug, Clone, PartialEq)]
pub enum ConvertTransparencyValue {
    /// The key is present and its value is None.
    None,
    /// Byte-table transparency is not representable as one color key.
    Bytes(Vec<u8>),
    /// A scalar numeric pixel value.
    Scalar(PaletteColorComponent),
    /// A tuple passed through Pillow's native pixel or palette-color path.
    Tuple(Vec<PaletteColorComponent>),
    /// A non-tuple Python sequence supplied to ImagingCore's key parser.
    Sequence(Vec<PaletteColorComponent>),
    /// A key resolved through a temporary mode-specific P palette callback.
    PaletteColor(Option<ConvertPaletteColor>),
    /// A value that is not one of the supported built-in forms.
    Other(String),
    /// A transparency key parsed with ImagingCore's scalar/three-component
    /// contract at the Python boundary.
    ImagingCoreKey(ImagingCoreTransparencyKey),
}

/// A resolved palette entry kept in its native channel layout across the
/// Python callback boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertPaletteColor {
    /// Three-channel RGB entry.
    Rgb([u8; 3]),
    /// Four-channel RGBA entry.
    Rgba([u8; 4]),
    /// Four-channel CMYK entry.
    Cmyk([u8; 4]),
}

impl ConvertPaletteColor {
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(crate) fn rgb(self) -> [u8; 3] {
        match self {
            Self::Rgb(color) => color,
            Self::Rgba([red, green, blue, _]) => [red, green, blue],
            Self::Cmyk(color) => crate::color::cmyk_pixel_to_rgb(color),
        }
    }
}

/// Core preflight result for transparency before Convert palette selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConvertTransparencyPreflight {
    /// Keep the copied info transparency value without a color remap.
    Preserve,
    /// Remove byte-table transparency and warn at the Python adapter boundary.
    RemoveByteTable,
    /// Convert the transparent source pixel to RGB before the destination route.
    RgbKey {
        /// RGB value produced by Pillow's native conversion sequence.
        rgb: [u8; 3],
        /// Source info background, when it reserves a palette slot.
        background_index: Option<u32>,
        /// Original scalar transparency value, when it reserves a palette slot.
        transparency_index: Option<u32>,
        /// Let the Python wrapper run ImagePalette allocation after `_new`.
        /// This preserves observable comparisons against arbitrary info values.
        host_palette_callback: bool,
    },
    /// Apply Pillow's P-mode byte-table alpha to a copied source palette.
    PaletteAlphaTable(Vec<u8>),
    /// Set one P-mode palette entry transparent on a copied source palette.
    PaletteAlphaIndex(i32),
    /// Promote a supported color-key mode to its alpha-bearing destination.
    TransparentAlpha {
        /// Scalar or RGB tuple key parsed at the host boundary.
        key: ImagingCoreTransparencyKey,
    },
}

/// Wrapper metadata change produced by Pillow-compatible Convert.
#[derive(Debug, Clone, PartialEq)]
pub enum ConvertTransparencyInfoDelta {
    /// Keep the metadata copied from the source image.
    Preserve,
    /// Remove the copied info transparency entry.
    Remove,
    /// Replace the copied info transparency entry with this value.
    Set(ImageInfoValue),
    /// Ask the Python wrapper to allocate this RGB key in the output P palette.
    SetPaletteColor {
        /// RGB color passed to `ImagePalette.getcolor`.
        rgb: [u8; 3],
        /// Pillow's adaptive-quantize route catches every callback exception;
        /// the ordinary palette route catches only `ValueError`.
        adaptive_palette: bool,
    },
}

/// Warning selected by the core operation for delivery by the host adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertWarning {
    /// No unused palette entry can represent the requested transparent color.
    TransparencyPaletteAllocation,
}

/// Image result plus the wrapper-owned metadata transition for Convert.
#[derive(Debug, Clone)]
pub struct ConvertInputOutput {
    /// Converted image pixels and palette.
    pub image: Image,
    /// Change to the Python-owned Image.info mapping.
    pub transparency_info: ConvertTransparencyInfoDelta,
    /// Warning to deliver after the output palette is prepared.
    pub warning: Option<ConvertWarning>,
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
#[derive(Debug, Clone)]
#[doc(hidden)]
pub enum ConvertInputPlan {
    /// The host adapter must evaluate `bool(matrix)` before planning continues.
    NeedsMatrixTruthiness,
    /// Same-mode conversion checked a truthy matrix twice in Pillow's wrapper.
    NeedsMatrixTruthinessAgain { first: bool },
    /// Return a mode-preserving copy without reading optional arguments.
    Copy,
    /// Execute a matrix conversion, then update RGB transparency metadata when
    /// Pillow's three-band source path requires it.
    Matrix {
        /// Matrix output mode.
        target_mode: Mode,
        /// Recompute wrapper transparency after pixel conversion succeeds.
        transform_transparency: bool,
    },
    /// Run RGBA-to-P's direct quantization branch, consuming the color count.
    Quantize { target_mode: Mode },
    /// Ask the host adapter for the wrapper-visible transparency value.
    NeedsTransparencyValue {
        target_mode: Mode,
        /// Input parser selected by the same source/target route contract.
        input_kind: ConvertTransparencyInputKind,
    },
    /// Compare the palette after transparency preflight.
    NeedsPaletteComparison {
        target_mode: Mode,
        transparency: Option<ConvertTransparencyPreflight>,
    },
    /// Select adaptive-palette quantization with its staged transparency value.
    AdaptiveQuantize {
        target_mode: Mode,
        transparency: Option<ConvertTransparencyPreflight>,
    },
    /// Execute an ordinary mode conversion, consuming only dither.
    Convert {
        target_mode: Mode,
        transparency: Option<ConvertTransparencyPreflight>,
    },
    /// Convert a supported source color key into an alpha-bearing target.
    TransparentAlpha {
        target_mode: Mode,
        key: ImagingCoreTransparencyKey,
    },
}

/// Host extraction contract for a staged Convert transparency value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[doc(hidden)]
pub enum ConvertTransparencyInputKind {
    /// ImagingCore.convert_transparent parses a scalar or RGB triple as C ints.
    ImagingCoreKey,
    /// P-to-alpha accepts only a bytes table or an `int` instance.
    PaletteAlpha,
    /// An L temporary pixel write consumes a scalar or one-element tuple.
    LumaPixelColor,
    /// An RGB temporary pixel write consumes a scalar or one-, three-, or four-tuple.
    RgbPixelColor,
    /// A P temporary palette resolves tuple colors through its palette object.
    PaletteColor,
}

impl ConvertInputPlan {
    /// Reports whether this plan consumes Pillow's requested palette size.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub const fn consumes_colors(&self) -> bool {
        matches!(self, Self::Quantize { .. } | Self::AdaptiveQuantize { .. })
    }
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
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_convert_input_plan(
        &self,
        mode: ConvertModeInput,
        matrix_truthiness: ConvertMatrixTruthiness,
        palette_adaptive: Option<bool>,
    ) -> Result<ConvertInputPlan, PilError> {
        let has_transparency = self.pending_transparency_info().is_some();
        let mut transparency = None;
        loop {
            let plan = self.prepare_convert_input_plan_with_transparency_preflight(
                mode.clone(),
                matrix_truthiness,
                palette_adaptive,
                has_transparency,
                transparency,
            )?;
            match plan {
                ConvertInputPlan::NeedsTransparencyValue { target_mode, .. } => {
                    transparency =
                        Some(self.prepare_convert_transparency_from_metadata(target_mode)?);
                }
                ready => return Ok(ready),
            }
        }
    }

    /// Selects Convert using wrapper-visible `Image.info` transparency state.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_convert_input_plan_with_transparency(
        &self,
        mode: ConvertModeInput,
        matrix_truthiness: ConvertMatrixTruthiness,
        palette_adaptive: Option<bool>,
        has_transparency: bool,
    ) -> Result<ConvertInputPlan, PilError> {
        self.prepare_convert_input_plan_with_transparency_preflight(
            mode,
            matrix_truthiness,
            palette_adaptive,
            has_transparency,
            None,
        )
    }

    /// Selects Convert after its wrapper-visible transparency value is staged.
    // TODO(pillow-rs#3ptb): parity-check P-to-alpha, transparent-alpha, and
    // matrix-transparency outputs and fault ordering.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn prepare_convert_input_plan_with_transparency_preflight(
        &self,
        mode: ConvertModeInput,
        matrix_truthiness: ConvertMatrixTruthiness,
        palette_adaptive: Option<bool>,
        has_transparency: bool,
        transparency: Option<ConvertTransparencyPreflight>,
    ) -> Result<ConvertInputPlan, PilError> {
        let source_mode = self.mode_identity()?;
        if let Some(plan) = invalid_mode_preflight(&mode, matrix_truthiness)? {
            return Ok(plan);
        }
        let Some(target_mode) =
            self.resolve_convert_target_mode(mode, source_mode, has_transparency)?
        else {
            return Ok(ConvertInputPlan::Copy);
        };
        if target_mode == source_mode {
            if let Some(plan) = same_mode_convert_preflight(target_mode, matrix_truthiness) {
                return Ok(plan);
            }
        }
        if let Some(plan) = matrix_convert_preflight(
            source_mode,
            target_mode,
            has_transparency,
            matrix_truthiness,
        )? {
            return Ok(plan);
        }
        if target_mode == source_mode {
            return Ok(ConvertInputPlan::Copy);
        }
        let transparency =
            staged_convert_transparency(source_mode, target_mode, has_transparency, transparency);
        let Some(transparency) = transparency else {
            let input_kind =
                super::transparency::convert_transparency_input_kind(source_mode, target_mode)
                    .ok_or_else(|| {
                        PilError::InternalError(
                            "Convert requested transparency outside a declared input route".into(),
                        )
                    })?;
            return Ok(ConvertInputPlan::NeedsTransparencyValue {
                target_mode,
                input_kind,
            });
        };
        if target_mode == Mode::ImagePalette8 && source_mode == Mode::ImageRgba8 {
            return Ok(ConvertInputPlan::Quantize { target_mode });
        }
        if target_mode == Mode::ImagePalette8 {
            return Ok(palette_convert_plan(
                target_mode,
                transparency,
                palette_adaptive,
            ));
        }
        if let Some(ConvertTransparencyPreflight::TransparentAlpha { key }) = &transparency {
            return Ok(ConvertInputPlan::TransparentAlpha {
                target_mode,
                key: *key,
            });
        }
        Ok(ConvertInputPlan::Convert {
            target_mode,
            transparency,
        })
    }

    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    fn resolve_convert_target_mode(
        &self,
        mode: ConvertModeInput,
        source_mode: Mode,
        has_transparency: bool,
    ) -> Result<Option<Mode>, PilError> {
        let target_mode = match mode {
            ConvertModeInput::None if source_mode != Mode::ImagePalette8 => return Ok(None),
            ConvertModeInput::None => {
                let default_mode = self
                    .palette_model()
                    .map(PaletteModel::raster_mode)
                    .unwrap_or(Mode::ImageRgb8);
                if default_mode == Mode::ImageRgb8 && has_transparency {
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
        Ok(Some(target_mode))
    }

    /// Executes a finalized Convert plan after the host extracts only values
    /// consumed by the selected branch.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn execute_convert_input_plan(
        &self,
        plan: ConvertInputPlan,
        matrix: Option<Vec<f64>>,
        dither: DitherInput,
        colors: Option<i64>,
        context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Image, PilError> {
        self.execute_convert_input_plan_with_metadata(plan, matrix, dither, colors, context)
            .map(|output| output.image)
    }

    /// Executes a finalized Convert plan and returns its wrapper metadata delta.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn execute_convert_input_plan_with_metadata(
        &self,
        plan: ConvertInputPlan,
        matrix: Option<Vec<f64>>,
        dither: DitherInput,
        colors: Option<i64>,
        context: Option<crate::compute::ExecutionContext>,
    ) -> Result<ConvertInputOutput, PilError> {
        let requested_backend = context.map(crate::compute::ExecutionContext::backend);
        self.execute_convert_input_plan_with_route(
            plan,
            matrix,
            dither,
            colors,
            context,
            requested_backend,
            self.backend(),
        )
    }

    /// Executes Convert while preserving its captured backend and CPU context.
    // TODO(pillow-rs#3ptb): verify eager request/context receipts for matrix,
    // quantize, and transparent-alpha routes; verify pin/context and selected
    // or fallback receipts when ordinary Convert remains lazy. Public Python
    // Convert loads its source first, so pinning must not reroute that load.
    // VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn execute_convert_input_plan_with_route(
        &self,
        plan: ConvertInputPlan,
        matrix: Option<Vec<f64>>,
        dither: DitherInput,
        colors: Option<i64>,
        context: Option<crate::compute::ExecutionContext>,
        requested_backend: Option<crate::compute::Backend>,
        pinned_backend: Option<crate::compute::Backend>,
    ) -> Result<ConvertInputOutput, PilError> {
        let execution_route = context
            .map(|context| crate::compute::CpuExecutionRoute::new(requested_backend, context));
        match plan {
            ConvertInputPlan::Copy => Ok(ConvertInputOutput {
                image: copy_with_context(self, context),
                transparency_info: ConvertTransparencyInfoDelta::Preserve,
                warning: None,
            }),
            ConvertInputPlan::Matrix { target_mode, .. } => {
                let matrix = matrix.ok_or_else(|| {
                    PilError::InternalError("Convert matrix plan has no matrix input".into())
                })?;
                validate_matrix_length(&matrix)?;
                let image = match execution_route {
                    Some(route) => {
                        self.convert_with_execution_route(target_mode, Some(matrix), None, route)
                    }
                    None => self.convert(target_mode, Some(matrix), None, None, None),
                }?;
                Ok(ConvertInputOutput {
                    image,
                    transparency_info: ConvertTransparencyInfoDelta::Preserve,
                    warning: None,
                })
            }
            ConvertInputPlan::Quantize { target_mode } => {
                if target_mode != Mode::ImagePalette8 {
                    return Err(PilError::InternalError(
                        "Convert quantize plan has a non-P target".into(),
                    ));
                }
                Ok(ConvertInputOutput {
                    image: self.execute_convert_quantize(colors, context, requested_backend)?,
                    transparency_info: ConvertTransparencyInfoDelta::Preserve,
                    warning: None,
                })
            }
            ConvertInputPlan::AdaptiveQuantize {
                target_mode,
                transparency,
            } => {
                if target_mode != Mode::ImagePalette8 {
                    return Err(PilError::InternalError(
                        "Convert adaptive quantize plan has a non-P target".into(),
                    ));
                }
                let image = self.execute_convert_quantize(colors, context, requested_backend)?;
                self.finish_convert_transparency(image, target_mode, transparency, true)
            }
            ConvertInputPlan::Convert {
                target_mode,
                transparency,
            } => {
                let dither = normalize_convert_dither(dither)?;
                let source =
                    self.convert_source_with_palette_alpha(transparency.as_ref(), context)?;
                let image = match execution_route {
                    Some(route) => source.convert_with_execution_route(
                        target_mode,
                        None,
                        dither.as_deref(),
                        route,
                    ),
                    None => source.convert(target_mode, None, dither.as_deref(), None, None),
                }?;
                self.finish_convert_transparency(image, target_mode, transparency, false)
            }
            ConvertInputPlan::TransparentAlpha { target_mode, key } => {
                let image = match context {
                    Some(context) => self.imaging_core_convert_transparent_with_execution_context(
                        target_mode,
                        key,
                        requested_backend,
                        pinned_backend,
                        context,
                    ),
                    None => self.imaging_core_convert_transparent(target_mode, key),
                }?;
                Ok(ConvertInputOutput {
                    image,
                    transparency_info: ConvertTransparencyInfoDelta::Remove,
                    warning: None,
                })
            }
            ConvertInputPlan::NeedsMatrixTruthiness
            | ConvertInputPlan::NeedsMatrixTruthinessAgain { .. }
            | ConvertInputPlan::NeedsTransparencyValue { .. }
            | ConvertInputPlan::NeedsPaletteComparison { .. } => Err(PilError::InternalError(
                "Convert input plan was executed before host staging completed".into(),
            )),
        }
    }

    /// Prepares a cloned source with the palette alpha requested by Convert.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn convert_source_with_palette_alpha(
        &self,
        transparency: Option<&ConvertTransparencyPreflight>,
        context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Image, PilError> {
        let mut source = match context {
            Some(context) => self.clone().with_execution_context(context),
            None => self.clone(),
        };
        match transparency {
            Some(ConvertTransparencyPreflight::PaletteAlphaTable(values)) => {
                source.putpalettealphas(values)?;
            }
            Some(ConvertTransparencyPreflight::PaletteAlphaIndex(index)) => {
                source.putpalettealpha(*index, 0)?;
            }
            _ => {}
        }
        Ok(source)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn execute_convert_quantize(
        &self,
        colors: Option<i64>,
        context: Option<crate::compute::ExecutionContext>,
        requested_backend: Option<crate::compute::Backend>,
    ) -> Result<Image, PilError> {
        let colors = i32::try_from(colors.unwrap_or(256)).map_err(|_| {
            PilError::OverflowError("Python int too large to convert to C int".into())
        })?;
        match context {
            Some(context) => self.quantize_with_visible_mode_and_route(
                Some(colors),
                None,
                None,
                crate::ops::quantize::QuantizePalette::None,
                None,
                None,
                crate::compute::CpuExecutionRoute::new(requested_backend, context),
            ),
            None => self.quantize_with_visible_mode_and_context(
                Some(colors),
                None,
                None,
                crate::ops::quantize::QuantizePalette::None,
                None,
                None,
                None,
            ),
        }
    }

    /// Applies Pillow's wrapper conversion contract under an injected route.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn first_matrix_truthiness(value: ConvertMatrixTruthiness) -> Option<bool> {
    match value {
        ConvertMatrixTruthiness::NotEvaluated => None,
        ConvertMatrixTruthiness::First(value) => Some(value),
        ConvertMatrixTruthiness::Repeated { first, .. } => Some(first),
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn invalid_mode_preflight(
    mode: &ConvertModeInput,
    matrix_truthiness: ConvertMatrixTruthiness,
) -> Result<Option<ConvertInputPlan>, PilError> {
    if !matches!(
        mode,
        ConvertModeInput::InvalidName(_) | ConvertModeInput::InvalidType(_)
    ) {
        return Ok(None);
    }
    let Some(matrix_truthy) = first_matrix_truthiness(matrix_truthiness) else {
        return Ok(Some(ConvertInputPlan::NeedsMatrixTruthiness));
    };
    if matrix_truthy {
        return Err(PilError::ValueError("illegal conversion".to_owned()));
    }
    Err(invalid_convert_mode_error(mode.clone()))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn same_mode_convert_preflight(
    target_mode: Mode,
    matrix_truthiness: ConvertMatrixTruthiness,
) -> Option<ConvertInputPlan> {
    match matrix_truthiness {
        ConvertMatrixTruthiness::NotEvaluated => Some(ConvertInputPlan::NeedsMatrixTruthiness),
        ConvertMatrixTruthiness::First(false) => Some(ConvertInputPlan::Copy),
        ConvertMatrixTruthiness::First(true) => {
            Some(ConvertInputPlan::NeedsMatrixTruthinessAgain { first: true })
        }
        ConvertMatrixTruthiness::Repeated { second: false, .. } => {
            Some(ConvertInputPlan::Convert {
                target_mode,
                transparency: None,
            })
        }
        ConvertMatrixTruthiness::Repeated { second: true, .. } => None,
    }
}

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn matrix_convert_preflight(
    source_mode: Mode,
    target_mode: Mode,
    has_transparency: bool,
    matrix_truthiness: ConvertMatrixTruthiness,
) -> Result<Option<ConvertInputPlan>, PilError> {
    let Some(matrix_truthy) = first_matrix_truthiness(matrix_truthiness) else {
        return Ok(Some(ConvertInputPlan::NeedsMatrixTruthiness));
    };
    if !matrix_truthy {
        return Ok(None);
    }
    // NO_PILLOW_SUPPORT: Rust also permits direct matrix conversion to LAB.
    if matches!(
        target_mode,
        Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImageLab8
    ) {
        return Ok(Some(ConvertInputPlan::Matrix {
            target_mode,
            transform_transparency: has_transparency
                && source_mode.band_count() == 3
                && target_mode != Mode::ImageLab8,
        }));
    }
    Err(PilError::ValueError("illegal conversion".to_owned()))
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn staged_convert_transparency(
    source_mode: Mode,
    target_mode: Mode,
    has_transparency: bool,
    transparency: Option<ConvertTransparencyPreflight>,
) -> Option<Option<ConvertTransparencyPreflight>> {
    if !has_transparency
        || super::transparency::convert_transparency_input_kind(source_mode, target_mode).is_none()
    {
        return Some(None);
    }
    transparency.map(Some)
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
fn palette_convert_plan(
    target_mode: Mode,
    transparency: Option<ConvertTransparencyPreflight>,
    palette_adaptive: Option<bool>,
) -> ConvertInputPlan {
    match palette_adaptive {
        None => ConvertInputPlan::NeedsPaletteComparison {
            target_mode,
            transparency,
        },
        Some(true) => ConvertInputPlan::AdaptiveQuantize {
            target_mode,
            transparency,
        },
        Some(false) => ConvertInputPlan::Convert {
            target_mode,
            transparency,
        },
    }
}

// VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
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

// VERIFY: source=reviewed@2026-11-24; parity=pending; benchmark=pending
fn validate_matrix_length(matrix: &[f64]) -> Result<(), PilError> {
    if matches!(matrix.len(), 4 | 12) {
        Ok(())
    } else {
        Err(PilError::TypeError(
            "matrix must be tuple of length 4 or 12".to_owned(),
        ))
    }
}
