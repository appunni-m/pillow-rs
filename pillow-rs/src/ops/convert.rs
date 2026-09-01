use crate::color;
use crate::error::PilError;
use crate::image::{Image, PipelineOps};
use crate::pipeline::{ColorMode, DitherMethod, PipelineOp};
use crate::raster::{DynamicImage, GenericImageView};

/// Parses a Pillow mode string into a pipeline color mode.
///
/// # Errors
///
/// Returns [`PilError::ValueError`] when `s` is not one of the modes supported
/// by core conversion.
pub fn parse_mode(s: &str) -> Result<ColorMode, PilError> {
    match s {
        "L" => Ok(ColorMode::L),
        "LA" => Ok(ColorMode::LA),
        "RGB" => Ok(ColorMode::RGB),
        "RGBA" => Ok(ColorMode::RGBA),
        "CMYK" => Ok(ColorMode::CMYK),
        "YCbCr" => Ok(ColorMode::YCbCr),
        "HSV" => Ok(ColorMode::HSV),
        "I" => Ok(ColorMode::I),
        "F" => Ok(ColorMode::F),
        "P" => Ok(ColorMode::P),
        "1" => Ok(ColorMode::Mode1),
        _ => Err(PilError::ValueError(format!("Unknown mode: {}", s))),
    }
}

fn is_luma16_mode(mode: &str) -> bool {
    matches!(mode, "I;16" | "I;16L" | "I;16B" | "I;16N")
}

/// Modes that require special handling when converting FROM them.
/// These modes store pixel data in a non-standard interpretation within
/// standard DynamicImage variants (e.g., CMYK values stored as RGBA).
fn is_nonstandard_mode(mode: &str) -> bool {
    matches!(mode, "CMYK" | "HSV" | "YCbCr" | "I" | "F" | "P" | "PA")
}

/// Extracts the Y band of a YCbCr image as luma bytes.
///
/// Pillow's C converter (`src/libImaging/Convert.c`) maps YCbCr to L and "1"
/// through the Y channel directly rather than through the RGB luminance.
fn ycbcr_luma8(img: &crate::raster::DynamicImage) -> crate::raster::GrayImage {
    let rgb = img.to_rgb8();
    crate::raster::GrayImage::from_fn(rgb.width(), rgb.height(), |x, y| {
        crate::raster::Luma([rgb.get_pixel(x, y)[0]])
    })
}

/// Per-entry alpha table Pillow applies when converting a palette image to an
/// alpha band: a single transparent index becomes 0 with everything else 255,
/// a PNG `tRNS` table is used verbatim.
fn palette_alpha_for_convert(img: &Image) -> Option<Vec<u8>> {
    if let Some(transparency) = img.pending_palette_transparency() {
        return match transparency {
            crate::image::PaletteTransparency::Index(index) => {
                let mut table = vec![255u8; 256];
                table[usize::from(index)] = 0;
                Some(table)
            }
            crate::image::PaletteTransparency::Table(alpha) => Some(alpha),
        };
    }
    // `putpalette(..., rawmode="RGBA")` stores attached alpha on the
    // committed palette rather than in pending `info["transparency"]`.
    // Pillow's P->LA conversion consumes both representations.
    img.palette_alpha().filter(|alpha| !alpha.is_empty())
}

fn parse_dither(s: Option<&str>) -> Result<Option<DitherMethod>, PilError> {
    match s {
        Some("NONE") | Some("none") => Ok(Some(DitherMethod::None)),
        Some("FLOYDSTEINBERG") | Some("floydsteinberg") => Ok(Some(DitherMethod::FloydSteinberg)),
        None => Ok(Some(DitherMethod::FloydSteinberg)), // PIL default: FloydSteinberg dither
        // Pillow's Python converter rejects string dither values before the C
        // converter interprets the integer enum.
        _ => Err(PilError::TypeError(
            "'str' object cannot be interpreted as an integer".into(),
        )),
    }
}

/// Host-neutral Python-facing dither input.
#[derive(Debug, Clone)]
pub enum PythonDitherInput {
    /// No dither argument was supplied.
    None,
    /// Python integer enum; zero disables dithering and non-zero selects
    /// Floyd-Steinberg.
    Integer(u32),
    /// Python string, which Pillow rejects for this entry point.
    Name(String),
    /// A value of another host type. The type name is used in Pillow's
    /// compatibility diagnostic.
    Invalid(String),
}

/// Validates a Python-facing dither argument that arrived as a string.
///
/// Pillow's Python API accepts the dither enum as an integer, even though the
/// shared Rust/JavaScript API uses symbolic strings internally. Parse the
/// symbolic value through the same core helper, then preserve Pillow's Python
/// type error for the host string input.
///
/// # Errors
///
/// Always returns [`PilError::TypeError`] because Python strings are not valid
/// dither enum arguments.
pub fn validate_python_convert_dither(value: &str) -> Result<(), PilError> {
    let _ = parse_dither(Some(value));
    Err(PilError::TypeError(
        "'str' object cannot be interpreted as an integer".into(),
    ))
}

/// Normalizes a Python-facing dither value into the symbolic core form.
pub fn normalize_python_convert_dither(
    value: PythonDitherInput,
) -> Result<Option<String>, PilError> {
    match value {
        PythonDitherInput::None => Ok(None),
        PythonDitherInput::Integer(value) => Ok(Some(if value == 0 {
            "NONE".to_owned()
        } else {
            "FLOYDSTEINBERG".to_owned()
        })),
        PythonDitherInput::Name(value) => validate_python_convert_dither(&value).map(|_| None),
        PythonDitherInput::Invalid(type_name) => Err(PilError::TypeError(format!(
            "'{type_name}' object cannot be interpreted as an integer"
        ))),
    }
}

/// Host-neutral destination mode input for the Python conversion wrapper.
#[derive(Debug, Clone)]
pub enum PythonConvertModeInput {
    /// No destination mode was supplied.
    None,
    /// A destination mode name.
    Name(String),
    /// A value of another host type.
    Invalid(String),
}

/// Host-neutral palette input for the Python conversion wrapper.
#[derive(Debug, Clone)]
pub enum PythonConvertPaletteInput {
    /// No palette object was supplied.
    None,
    /// A symbolic palette name.
    Name(String),
    /// A value of another host type.
    Invalid(String),
}

/// Pillow-compatible image mode conversion methods.
impl Image {
    /// Applies Python's default-mode, palette, and matrix validation before
    /// entering the shared conversion implementation.
    pub fn convert_with_input(
        &self,
        mode: PythonConvertModeInput,
        matrix: Option<Vec<f64>>,
        dither: PythonDitherInput,
        palette: PythonConvertPaletteInput,
        colors: Option<u32>,
    ) -> Result<Image, PilError> {
        let source_mode = self.mode()?;
        let target_mode = match mode {
            PythonConvertModeInput::None => {
                if source_mode != "P" {
                    return Ok(self.copy());
                }
                let mut target = self.palette_mode().unwrap_or("RGB").to_owned();
                if target == "RGB" && self.has_transparency_data() {
                    target = "RGBA".to_owned();
                }
                target
            }
            PythonConvertModeInput::Name(target) => target,
            PythonConvertModeInput::Invalid(type_name) => {
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
        if let Some(matrix) = matrix.as_ref() {
            if !matches!(matrix.len(), 4 | 12) {
                return Err(PilError::TypeError(format!(
                    "argument 2 must be sequence of length 12, not {}",
                    matrix.len()
                )));
            }
        }

        let palette = match palette {
            PythonConvertPaletteInput::None => None,
            PythonConvertPaletteInput::Name(name) => Some(name),
            // Pillow accepts and ignores palette values for the public
            // conversion paths exercised here; only the target mode and
            // dither/matrix inputs affect conversion dispatch.
            PythonConvertPaletteInput::Invalid(_) => None,
        };
        // Keep Python's dither coercion in the Rust core. Pillow ignores the
        // argument on a same-mode no-op and on matrix conversions, and reports
        // matrix/mode errors before it attempts to interpret the dither enum.
        let dither = if matrix.is_some() {
            None
        } else {
            normalize_python_convert_dither(dither)?
        };
        self.convert(
            &target_mode,
            matrix,
            dither.as_deref(),
            palette.as_deref(),
            colors,
        )
    }

    /// Converts this image to another Pillow mode.
    ///
    /// # Inputs
    ///
    /// - `mode`: destination Pillow mode, such as `"L"`, `"RGB"`, `"RGBA"`,
    ///   `"CMYK"`, `"HSV"`, `"YCbCr"`, `"I"`, `"F"`, `"P"`, or `"1"`.
    /// - `matrix`: optional Pillow conversion matrix for immediate conversion.
    ///   Four values convert RGB input to luma; twelve values convert RGB input
    ///   through a 3x4 color matrix.
    /// - `dither`: optional dither name for binary and palette-like conversion.
    ///
    /// # Returns
    ///
    /// A new [`Image`] tagged with the destination mode. Matrix conversions
    /// execute immediately; mode-only conversions may be represented lazily in
    /// the pipeline.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when the mode, matrix, dither option, or source
    /// image data is invalid.
    pub fn convert(
        &self,
        mode: &str,
        matrix: Option<Vec<f64>>,
        dither: Option<&str>,
        _palette: Option<&str>,
        _colors: Option<u32>,
    ) -> Result<Image, PilError> {
        // Validate the target before source-specific conversion dispatch. The
        // Python wrapper used to reject unknown modes first, which made this
        // public Rust error path unreachable and allowed an unknown target to
        // slip through for some non-standard source modes. PA is handled by
        // the explicit palette-alpha path below but is not a ColorMode enum.
        if mode != "PA" && !is_luma16_mode(mode) {
            parse_mode(mode).map_err(|_| PilError::ValueError("image has wrong mode".into()))?;
        }

        // PIL: convert() without mode arg keeps same mode for most types,
        // but converts P→RGB (palette images default to RGB when no mode given).
        let src_mode = self.mode()?;

        // Pillow stores bilevel "1" pixels as 0/255; our core keeps the raw
        // 0/1 bytes, so every conversion FROM "1" must map 1 -> 255 first
        // (Pillow's convert treats "1" as "L" through the luma path).
        if src_mode == "1" && mode != "1" {
            let img = self.materialize()?;
            let mut gray = crate::raster::GrayImage::new(img.width(), img.height());
            for (op, ip) in gray.pixels_mut().zip(img.to_luma8().pixels()) {
                op[0] = if ip[0] != 0 { 255 } else { 0 };
            }
            let expanded = crate::raster::DynamicImage::ImageLuma8(gray);
            return Image::from_dynamic(expanded, None).convert(mode, matrix, dither, None, None);
        }

        // Matrix-based conversion must be executed even when the requested
        // mode equals the source mode. Pillow applies the matrix before its
        // same-mode fast path; returning a copy first silently discarded the
        // matrix and made RGB->RGB conversion diverge.
        if let Some(mat) = matrix {
            let img = self.materialize()?;
            return convert_with_matrix(&img, mode, &mat)
                .map(|result| Image::from_dynamic(result, explicit_mode_for(mode)));
        }

        if mode == src_mode && src_mode != "P" {
            return Ok(self.copy());
        }

        // Pillow's typed I;16 converters consume an 8-bit luma sample as the
        // same numeric unsigned word; they do not expand 17 to 0x1111 like
        // the generic image-raster u8→u16 conversion.  Keep this destination
        // path eager so the logical mode tag and declared byte order survive
        // without adding a byte-oriented ColorMode variant to PipelineOp.
        if is_luma16_mode(mode) {
            if src_mode == "P" {
                // Pillow's palette converter has no direct I;16 destination
                // and reports this exact conversion error before Paste.c.
                return Err(PilError::ValueError("conversion not supported".into()));
            }
            let samples = if src_mode == "I" && mode != "I;16N" {
                // Pillow's I→I;16/I;16L/I;16B converter is the one typed
                // scalar path: clamp the signed 32-bit sample to 0..65535.
                // I;16N on the little-endian oracle has only the byte-domain
                // converter, so it intentionally falls through to L below.
                let image = self.materialize()?;
                let (width, height) = image.dimensions();
                let raw = image.as_bytes();
                let samples = raw.chunks_exact(4).map(|bytes| {
                    let value = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
                    value.clamp(0, i32::from(u16::MAX)) as u16
                });
                crate::raster::ImageBuffer::from_raw(width, height, samples.collect()).ok_or_else(
                    || PilError::InternalError("I;16 conversion buffer shape mismatch".into()),
                )?
            } else {
                let luma = self
                    .convert("L", None, None, None, None)?
                    .materialize()?
                    .to_luma8();
                let (width, height) = luma.dimensions();
                crate::raster::ImageBuffer::from_fn(width, height, |x, y| {
                    crate::raster::Luma([u16::from(luma.get_pixel(x, y)[0])])
                })
            };
            return Ok(Image::from_dynamic(
                DynamicImage::ImageLuma16(samples),
                Some(mode.to_owned()),
            ));
        }
        // P-mode same-mode: PIL defaults to RGB
        let mode = if mode == src_mode && src_mode == "P" {
            "RGB"
        } else {
            mode
        };

        // Handle conversion from non-standard modes (CMYK, HSV, YCbCr, I, F, P).
        // These modes store pixel data in standard DynamicImage containers but with
        // a different interpretation (e.g., CMYK values stored as RGBA). We must
        // materialize first and convert using PIL's exact algorithms.
        // Lazy Bytes images report no explicit mode; fall back to the decoded
        // mode so non-standard sources (P, CMYK, I, F, HSV, YCbCr) take the
        // palette/transparency-aware conversion path even before load().
        let effective_src_mode = self
            .explicit_mode()
            .map(str::to_owned)
            .unwrap_or_else(|| src_mode.clone());
        let effective_src_mode_name = effective_src_mode.as_str();

        // Pillow's Image.convert first tries the direct I/F converter when
        // one exists, then retries unsupported destinations through the
        // source mode's base type, which is L.  Normalizing these scalar
        // modes through RGB changes Pillow's fixed-point behavior (for
        // example, RGB(11, 11, 11) has Y=10 while F(11.5) truncates to an
        // L value of 11 before a YCbCr conversion).
        // PA is a dedicated indexed-plus-alpha destination; route I/F through
        // convert_to_palette_alpha below so scalar samples use its direct
        // clamped-index conversion instead of the generic L fallback.
        if matches!(effective_src_mode_name, "I" | "F") && mode != "PA" {
            let img = self.materialize()?;
            let result = match (effective_src_mode_name, mode) {
                ("I", "F") => color::i_to_f(&img),
                ("F", "I") => color::f_to_i(&img),
                _ => {
                    let luma = if effective_src_mode_name == "I" {
                        color::i_to_l(&img)
                    } else {
                        color::f_to_l(&img)
                    };
                    return Image::from_dynamic(luma, None)
                        .convert(mode, matrix, dither, None, _colors);
                }
            };
            return Ok(Image::from_dynamic(result, explicit_mode_for(mode)));
        }

        // Pillow's Convert.c first reinterprets non-standard source storage
        // as RGB whenever the destination is another non-standard family.
        // The deferred scalar converter only sees the destination mode, so
        // passing raw CMYK/HSV/YCbCr/I/F bytes through would treat those bytes
        // as ordinary RGB samples. Normalize the source through the public
        // RGB representation before dispatching HSV, YCbCr, P, I, or F.
        // CMYK->I/F retains its dedicated exact path below.
        if is_nonstandard_mode(effective_src_mode_name)
            && matches!(mode, "HSV" | "YCbCr" | "P" | "I" | "F")
            && mode != effective_src_mode_name
            && !(effective_src_mode_name == "CMYK" && matches!(mode, "I" | "F"))
        {
            let palette = self.palette();
            let img = self.materialize()?;
            let converted = if effective_src_mode_name == "PA" {
                crate::image::expand_palette_alpha(
                    &img.to_luma_alpha8(),
                    palette.as_deref().unwrap_or_default(),
                )
            } else {
                color::convert_from_nonstandard(effective_src_mode_name, &img, palette.as_deref())
                    .unwrap_or_else(|| img.to_rgb8().into())
            };
            let rgb_source = Image::from_dynamic(converted, None);
            return rgb_source.convert(mode, matrix, dither, None, _colors);
        }

        // `PA` is a real Pillow destination mode even though it is not one
        // of the scalar pipeline modes.  Its conversion is not a generic
        // mode tag: Pillow builds palette indices and a second, per-pixel
        // alpha band.  Resolve it eagerly so standard and non-standard source
        // modes follow the same public conversion contract.
        if mode == "PA" {
            let dither_enum = parse_dither(dither)?;
            return convert_to_palette_alpha(self, effective_src_mode_name, dither_enum);
        }

        // Pillow routes unsigned 16-bit luma to the grayscale CMYK branch:
        // C=M=Y=0 and K=255 minus the clipped luma sample. The ordinary
        // packed converter cannot distinguish this source from RGB storage,
        // so materialize the native samples before queuing the operation.
        if mode == "CMYK"
            && matches!(
                effective_src_mode_name,
                "I;16" | "I;16L" | "I;16B" | "I;16N"
            )
        {
            let gray = self.materialize()?.to_luma8();
            let (width, height) = gray.dimensions();
            let mut result = crate::raster::RgbaImage::new(width, height);
            for (output, input) in result.pixels_mut().zip(gray.pixels()) {
                *output = crate::raster::Rgba([0, 0, 0, 255u8.saturating_sub(input[0])]);
            }
            return Ok(Image::from_dynamic(
                DynamicImage::ImageRgba8(result),
                explicit_mode_for(mode),
            ));
        }

        let target_is_standard = !is_nonstandard_mode(mode);
        // Non-standard sources must be materialized and converted to RGB
        // before reaching a standard target OR a CMYK target (Pillow's
        // CMYK inverse runs on the RGB values).  CMYK->CMYK is identity.
        if is_nonstandard_mode(effective_src_mode_name)
            && (target_is_standard
                || mode == "CMYK"
                || (mode == "PA" && effective_src_mode_name == "P"))
            // PA→RGB can use the ordinary native byte converter after the
            // pipeline evaluator expands its indexed samples through the
            // retained palette.  Keep this one conversion lazy so SIMD/GPU
            // adapters receive the same RGBA layout as every other RGB
            // source; all other PA destinations still need the eager,
            // mode-specific conversion below.
            && !(effective_src_mode_name == "PA" && mode == "RGB")
        {
            let src_mode = effective_src_mode_name;
            // Extract palette before materializing (P-mode palette may be on Pipeline)
            let palette = self.palette();
            let img = self.materialize()?;
            let converted = if src_mode == "PA" {
                // PA stores a palette index and a per-pixel alpha byte.
                // Expand both before grayscale/CMYK conversion; treating
                // the index as luma makes an unpaletted PA image produce
                // visible color where Pillow correctly returns black.
                crate::image::expand_palette_alpha(
                    &img.to_luma_alpha8(),
                    palette.as_deref().unwrap_or_default(),
                )
            } else {
                color::convert_from_nonstandard(src_mode, &img, palette.as_deref())
                    .unwrap_or_else(|| img.to_rgb8().into())
            };
            // For mode "L" etc., derive from the RGB result.
            let result = if mode == "CMYK" {
                // I/F sources return through the scalar dispatch above before
                // this fallback, so this active path receives RGB-family
                // representations and applies the RGB inverse directly.
                DynamicImage::ImageRgba8(crate::color::rgb_to_cmyk_inverse(&converted.to_rgb8()))
            } else if mode == "L" || mode == "LA" {
                if mode == "L" && src_mode == "YCbCr" {
                    // Pillow's C converter maps YCbCr to L through the Y
                    // band directly, not through the RGB luma.
                    DynamicImage::ImageLuma8(ycbcr_luma8(&img))
                } else if mode == "LA" && src_mode == "YCbCr" {
                    // Convert.c's ycbcr2la likewise copies the Y band and
                    // installs an opaque alpha byte. Reconstructing RGB
                    // first would change the fixed-point Y value and expose
                    // a storage byte as alpha.
                    let gray = ycbcr_luma8(&img);
                    let (width, height) = gray.dimensions();
                    let mut la = crate::raster::GrayAlphaImage::new(width, height);
                    for (output, gray_pixel) in la.pixels_mut().zip(gray.pixels()) {
                        output[0] = gray_pixel[0];
                        output[1] = 255;
                    }
                    DynamicImage::ImageLumaA8(la)
                } else if mode == "L" {
                    DynamicImage::ImageLuma8(color::pil_grayscale(&converted)?)
                } else {
                    let mut la = if src_mode == "PA" {
                        // PA expansion carries the source alpha in the RGBA
                        // result.  Keep that band when Pillow converts PA to
                        // LA; the generic grayscale-alpha helper intentionally
                        // creates an opaque alpha band for RGB-family inputs.
                        let gray = color::pil_grayscale(&converted)?;
                        let rgba = converted.to_rgba8();
                        let (width, height) = gray.dimensions();
                        let mut result = crate::raster::GrayAlphaImage::new(width, height);
                        for ((output, gray_pixel), rgba_pixel) in
                            result.pixels_mut().zip(gray.pixels()).zip(rgba.pixels())
                        {
                            output[0] = gray_pixel[0];
                            output[1] = rgba_pixel[3];
                        }
                        result
                    } else {
                        color::pil_grayscale_alpha(&converted)?
                    };
                    if src_mode == "P" {
                        // Pillow carries palette transparency into the LA
                        // alpha band (putpalettealpha before converting).
                        let indices = img.to_luma8();
                        if let Some(table) = palette_alpha_for_convert(self) {
                            for (op, ip) in la.pixels_mut().zip(indices.pixels()) {
                                op[1] = table.get(usize::from(ip[0])).copied().unwrap_or(255);
                            }
                        }
                    }
                    DynamicImage::ImageLumaA8(la)
                }
            } else if mode == "RGB" {
                // PA expansion carries the per-pixel alpha needed by RGBA
                // conversion, but Pillow's RGB conversion drops that band.
                DynamicImage::ImageRgb8(converted.to_rgb8())
            } else if mode == "RGBA" {
                // P sources with palette alpha keep per-entry alpha when
                // converting to RGBA; the RGB-only nonstandard path would
                // force every pixel opaque.
                if src_mode == "P" {
                    let palette_alpha = self.palette_alpha().unwrap_or_default();
                    if !palette_alpha.is_empty() {
                        let indices = img.to_luma8();
                        let pal = palette.as_deref().unwrap_or_default();
                        return Ok(Image::from_dynamic(
                            crate::image::expand_palette(&indices, pal, &palette_alpha),
                            explicit_mode_for(mode),
                        ));
                    }
                }
                DynamicImage::ImageRgba8(converted.to_rgba8())
            } else {
                converted
            };
            if mode != "1" {
                return Ok(Image::from_dynamic(result, explicit_mode_for(mode)));
            }
            // Binary mode "1" falls through to the shared threshold/dither
            // path below, which re-derives the grayscale from the
            // non-standard source and applies Pillow's dither policy.
        }

        let dither_enum = parse_dither(dither)?;

        // Special case: converting to binary mode "1" — must eagerly execute
        // because the pipeline's scalar::convert doesn't handle binary threshold/dither.
        if mode == "1" {
            let img = self.materialize()?;
            // Use truncated grayscale (PIL uses integer truncation, not rounding)
            let gray = if effective_src_mode == "CMYK" {
                crate::color::cmyk_to_grayscale_truncate(&img)?
            } else if is_nonstandard_mode(&effective_src_mode) {
                let palette = self.palette();
                let rgb = crate::color::convert_from_nonstandard(
                    &effective_src_mode,
                    &img,
                    palette.as_deref(),
                )
                .unwrap_or_else(|| img.to_rgb8().into());
                crate::color::pil_grayscale_truncate(&rgb)?
            } else {
                crate::color::pil_grayscale_truncate(&img)?
            };
            let (w, h) = gray.dimensions();
            let mut out = crate::raster::GrayImage::new(w, h);
            // Pillow's indexed conversion path thresholds P/PA palette colors
            // directly for mode "1", even when Floyd-Steinberg is requested
            // or implied by the default. Do not dither after expanding them.
            let use_threshold = matches!(dither_enum, Some(DitherMethod::None))
                || matches!(effective_src_mode.as_str(), "P" | "PA");
            if use_threshold {
                // Threshold at 128 (PIL: pixel >= 128 -> 255, else 0)
                for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                    op[0] = if gp[0] >= 128 { 255 } else { 0 };
                }
            } else {
                // Floyd-Steinberg dither (PIL's tobilevel with error diffusion)
                // Uses PIL's exact error propagation pattern (scaled by 16).
                // Key detail: after the inner loop, PIL writes errors[w] = l0
                // so the next row's last pixel reads the down/down-right error
                // from this row's last pixel.
                // For RGB source, compute luminance inline using PIL's formula
                // (299*R + 587*G + 114*B) / 1000 to exactly match PIL behavior.
                // The pre-computed grayscale uses >> 16 which differs for 5 out
                // of 16M RGB values.
                // Non-standard sources (HSV/YCbCr) are stored in RGB
                // containers but their byte values are not RGB; their
                // true luminance is in the pre-computed `gray`.
                let source_is_nonstandard = is_nonstandard_mode(&effective_src_mode);
                let is_rgb =
                    matches!(img.color(), crate::raster::ColorType::Rgb8) && !source_is_nonstandard;
                let mut errors = vec![0i32; (w + 1) as usize];
                let wu = w as usize;
                if is_rgb {
                    let rgb = img.to_rgb8();
                    let rgb_raw = rgb.into_raw();
                    for y in 0..h as usize {
                        let mut l = 0i32;
                        let mut l0: i32 = 0;
                        let mut l1: i32 = 0;
                        let row_base = y * wu * 3;
                        for x in 0..wu {
                            let r = rgb_raw[row_base + x * 3] as i32;
                            let g = rgb_raw[row_base + x * 3 + 1] as i32;
                            let b = rgb_raw[row_base + x * 3 + 2] as i32;
                            let lum = (299 * r + 587 * g + 114 * b) / 1000;
                            let acc = l + errors[x + 1];
                            let v = (lum + acc / 16).clamp(0, 255);
                            let new = if v > 128 { 255i32 } else { 0i32 };
                            out.get_pixel_mut(x as u32, y as u32)[0] = new as u8;
                            l = v - new;
                            let l2 = l;
                            let d2 = l + l;
                            l += d2;
                            errors[x] = l + l0;
                            l += d2;
                            l0 = l + l1;
                            l1 = l2;
                            l += d2;
                        }
                        errors[wu] = l0;
                    }
                } else {
                    let src: Vec<i32> = gray.pixels().map(|p| p[0] as i32).collect();
                    for y in 0..h as usize {
                        let mut l = 0i32;
                        let mut l0: i32 = 0;
                        let mut l1: i32 = 0;
                        for x in 0..wu {
                            let idx = y * wu + x;
                            let acc = l + errors[x + 1];
                            let v = src[idx] + acc / 16;
                            let v = v.clamp(0, 255);
                            let new = if v > 128 { 255i32 } else { 0i32 };
                            out.get_pixel_mut(x as u32, y as u32)[0] = new as u8;
                            l = v - new;
                            let l2 = l;
                            let d2 = l + l;
                            l += d2;
                            errors[x] = l + l0;
                            l += d2;
                            l0 = l + l1;
                            l1 = l2;
                            l += d2;
                        }
                        errors[wu] = l0;
                    }
                }
            }
            return Ok(Image::from_dynamic(
                DynamicImage::ImageLuma8(out),
                Some("1".to_string()),
            ));
        }

        // Special case: converting to P-mode uses PIL's default WEB palette
        // with Floyd-Steinberg dither, not median cut quantize. We eagerly execute
        // here so the palette is stored on the result Pipeline, enabling subsequent
        // convert("RGB") operations to do correct palette lookups.
        if mode == "P" {
            if src_mode == "RGBA" {
                // Pillow routes RGBA->P through FASTOCTREE rather than the
                // fixed WEB palette used for RGB-family sources without an
                // alpha band.  Returning the quantizer's indexed pipeline
                // also preserves its palette metadata for later conversions.
                return self.quantize(256, 0, None, true, 2);
            }
            use crate::ops::quantize::web_palette_quantize;
            use std::sync::Arc;

            let img = self.materialize()?;
            let (w, h) = (img.width(), img.height());
            let (indices, palette_bytes) = if matches!(src_mode.as_str(), "1" | "L" | "LA") {
                // Pillow 12.2 Convert.c maps L/1 samples directly to P indices
                // and installs the identity grayscale palette. LA uses the
                // same luma-band path; its alpha is not part of a P result.
                // Web quantization would change the indices before mixed-mode
                // paste/composite.
                let indices = img.to_luma8().into_raw();
                let palette = (0u8..=u8::MAX)
                    .flat_map(|value| [value, value, value])
                    .collect();
                (indices, palette)
            } else {
                let rgb = img.to_rgb8();
                let rgb_raw = rgb.into_raw();
                let dither = !matches!(dither_enum, Some(DitherMethod::None));
                web_palette_quantize(&rgb_raw, w, h, dither)?
            };
            let mut out = crate::raster::GrayImage::new(w, h);
            for (i, pixel) in out.pixels_mut().enumerate() {
                pixel[0] = indices.get(i).copied().unwrap_or(0);
            }
            // Pillow retains the source EXIF record across mode conversion;
            // this matters when a converted indexed image is subsequently
            // passed to ImageOps.exif_transpose. Keep the encoded metadata on
            // the pipeline source so the public operation can remove the
            // orientation tag after transposing the palette indices.
            let source = crate::image::LoadedData {
                image: Arc::new(DynamicImage::ImageLuma8(out)),
                explicit_mode: Some("P".to_string()),
                decoded_mode: crate::raster::ColorType::L8.into(),
                palette: None,
                palette_alpha: None,
                source_format: self.source_format(),
                info: self.image_info(),
                exif: self.exif_metadata(),
            };
            return Ok(Image::Pipeline {
                source: Arc::new(Image::Loaded(source)),
                ops: PipelineOps::empty(),
                format: None,
                explicit_mode: Some("P".to_string()),
                backend: None,
                palette: Some(palette_bytes),
                palette_alpha: None,
                materialized: crate::image::materialization_cache(),
                shape: crate::image::pipeline_shape_cache(),
                mode: crate::image::pipeline_mode_cache(),
            });
        }

        // I/F targets keep their four-byte sample representation. Unlike
        // ordinary RGB-backed conversions, a CMYK source must first expand
        // C/M/Y/K into RGB; the deferred Convert op only receives the target
        // mode tag, so preserve this source-specific conversion here.
        if effective_src_mode_name == "CMYK" && matches!(mode, "I" | "F") {
            let img = self.materialize()?;
            let rgb = crate::color::cmyk_to_rgb(&img).to_rgb8();
            let (w, h) = rgb.dimensions();
            let mut out = crate::raster::RgbaImage::new(w, h);
            for (output, pixel) in out.pixels_mut().zip(rgb.pixels()) {
                let r = i32::from(pixel[0]);
                let g = i32::from(pixel[1]);
                let b = i32::from(pixel[2]);
                if mode == "I" {
                    let value = (19595i32 * r + 38470i32 * g + 7471i32 * b + 32768) >> 16;
                    *output = crate::raster::Rgba(value.to_le_bytes());
                } else {
                    let value = (r * 299 + g * 587 + b * 114) as f32 / 1000.0;
                    *output = crate::raster::Rgba(value.to_le_bytes());
                }
            }
            let result = crate::raster::DynamicImage::ImageRgba8(out);
            return Ok(Image::from_dynamic(result, explicit_mode_for(mode)));
        }

        let mode_enum = parse_mode(mode)?;
        // Pillow accepts a dither argument on convert(), but its standard
        // byte-mode converters do not consume it. Keep the descriptor free of
        // a phantom default Floyd-Steinberg value so the GPU contract can
        // dispatch the exact byte-to-byte kernel. Binary and palette targets
        // still retain the normalized dither because their conversion paths
        // use it above.
        let pipeline_dither = if matches!(mode, "P" | "1") {
            dither_enum
        } else {
            // Pillow's byte and scalar converters do not consume the dither
            // enum. Keeping the default Floyd-Steinberg value on CMYK/HSV/
            // YCbCr/I/F descriptors needlessly blocks their exact GPU byte
            // kernels and does not describe an operation the converter uses.
            None
        };
        let mut result = Image::push_op(
            self,
            PipelineOp::Convert {
                mode: mode_enum,
                matrix: None,
                dither: pipeline_dither,
            },
        );
        // Set explicit_mode on the pipeline for non-standard modes
        if let Some(em) = explicit_mode_for(mode) {
            if let Image::Pipeline {
                explicit_mode: em_field,
                ..
            } = &mut result
            {
                *em_field = Some(em.to_string());
            }
        }
        Ok(result)
    }
}

/// Convert a source image to Pillow's palette-plus-alpha representation.
///
/// Pillow's `Convert.c` treats `PA` as an indexed destination, not as a
/// spelling of `LA`: RGB-family and CMYK/HSV/YCbCr sources are mapped through
/// the default WEB palette, RGBA keeps the source alpha bytes, and scalar or
/// indexed sources retain their byte/index values. Scalar sources also receive
/// Pillow's identity grayscale palette, so a later `PA` expansion resolves an
/// index back to the original scalar value. Keeping those paths explicit also
/// avoids feeding `PA` into the ordinary scalar pipeline, which has no palette
/// storage to attach to its result.
fn identity_grayscale_palette() -> Vec<u8> {
    (0u8..=u8::MAX)
        .flat_map(|value| [value, value, value])
        .collect()
}

fn convert_to_palette_alpha(
    image: &Image,
    source_mode: &str,
    dither: Option<DitherMethod>,
) -> Result<Image, PilError> {
    let source = image.materialize()?;
    let (indices, palette, alpha) = match source_mode {
        "P" => {
            let indices = source.to_luma8();
            let table = palette_alpha_for_convert(image);
            let alpha = indices
                .pixels()
                .map(|pixel| {
                    table
                        .as_ref()
                        .and_then(|values| values.get(usize::from(pixel[0])))
                        .copied()
                        .unwrap_or(255)
                })
                .collect();
            (indices, image.palette(), alpha)
        }
        "1" | "L" | "LA" => {
            let indices = source.to_luma8();
            let alpha = vec![255; indices.width() as usize * indices.height() as usize];
            (indices, Some(identity_grayscale_palette()), alpha)
        }
        "I" => {
            // I->P is a direct clamped scalar-to-index conversion.  The
            // intermediate RGB helper preserves that value without applying
            // the I->L scaling formula used by ordinary grayscale conversion.
            let indices = color::i_to_rgb(&source).to_luma8();
            let alpha = vec![255; indices.width() as usize * indices.height() as usize];
            (indices, Some(identity_grayscale_palette()), alpha)
        }
        "F" => {
            // F->P truncates the clamped float value to an index, matching the
            // existing F->RGB helper because all three broadcast channels are
            // identical.
            let indices = color::f_to_rgb(&source).to_luma8();
            let alpha = vec![255; indices.width() as usize * indices.height() as usize];
            (indices, Some(identity_grayscale_palette()), alpha)
        }
        "RGBA" => {
            // Pillow's Python conversion splits RGBA into RGB and alpha,
            // quantizes only the RGB image, then merges the original alpha
            // band into PA.  Quantizing RGBA and moving transparent entries
            // afterward produces different palette-index order for valid
            // images with distinct transparent and visible colors.
            let rgba = source.to_rgba8();
            let alpha: Vec<u8> = rgba.pixels().map(|pixel| pixel[3]).collect();
            let (width, height) = rgba.dimensions();
            let mut rgb = crate::raster::RgbImage::new(width, height);
            for (output, input) in rgb.pixels_mut().zip(rgba.pixels()) {
                *output = crate::raster::Rgb([input[0], input[1], input[2]]);
            }
            let quantized =
                Image::from_dynamic(DynamicImage::ImageRgb8(rgb), Some("RGB".to_owned()))
                    // Image.convert("PA") calls RGB.quantize() without a method;
                    // RGB therefore uses MEDIANCUT (method 0), not RGBA's FASTOCTREE
                    // default.  The palette-index order is observable in PA bytes.
                    .quantize(256, 0, None, true, 0)?;
            let index_bytes = quantized.materialize()?.to_luma8().into_raw();
            let palette = quantized.palette();
            let indices = crate::raster::GrayImage::from_raw(width, height, index_bytes)
                .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
            (indices, palette, alpha)
        }
        "CMYK" | "HSV" | "YCbCr" => {
            let source_palette = image.palette();
            let rgb =
                color::convert_from_nonstandard(source_mode, &source, source_palette.as_deref())
                    .unwrap_or_else(|| source.to_rgb8().into())
                    .to_rgb8();
            let (width, height) = rgb.dimensions();
            let dither = !matches!(dither, Some(DitherMethod::None));
            let (indices, palette) = crate::ops::quantize::web_palette_quantize(
                &rgb.clone().into_raw(),
                width,
                height,
                dither,
            )?;
            let indices = crate::raster::GrayImage::from_raw(width, height, indices)
                .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
            let alpha = vec![255; width as usize * height as usize];
            (indices, Some(palette), alpha)
        }
        _ => {
            let rgb = source.to_rgb8();
            let (width, height) = rgb.dimensions();
            let dither = !matches!(dither, Some(DitherMethod::None));
            let (indices, palette) = crate::ops::quantize::web_palette_quantize(
                &rgb.clone().into_raw(),
                width,
                height,
                dither,
            )?;
            let indices = crate::raster::GrayImage::from_raw(width, height, indices)
                .ok_or_else(|| PilError::ValueError("palette conversion failed".to_owned()))?;
            let alpha = vec![255; width as usize * height as usize];
            (indices, Some(palette), alpha)
        }
    };

    let (width, height) = indices.dimensions();
    let index_bytes = indices.into_raw();
    let mut pa = crate::raster::GrayAlphaImage::new(width, height);
    for (position, output) in pa.pixels_mut().enumerate() {
        output[0] = index_bytes[position];
        output[1] = alpha.get(position).copied().unwrap_or(255);
    }

    Ok(Image::Loaded(crate::image::LoadedData {
        image: std::sync::Arc::new(DynamicImage::ImageLumaA8(pa)),
        explicit_mode: Some("PA".to_owned()),
        decoded_mode: crate::raster::ColorType::La8.into(),
        palette,
        palette_alpha: None,
        // Pillow's mode conversion returns a detached image and does not
        // carry the source decoder format or transient info dictionary into
        // the result.  Retain only EXIF, matching the existing P/PA path.
        source_format: None,
        info: None,
        exif: image.exif_metadata(),
    }))
}

fn explicit_mode_for(mode: &str) -> Option<String> {
    match mode {
        "1" | "P" | "CMYK" | "HSV" | "YCbCr" | "I" | "F" => Some(mode.to_string()),
        _ => None,
    }
}

fn convert_with_matrix(
    img: &crate::raster::DynamicImage,
    target_mode: &str,
    matrix: &[f64],
) -> Result<crate::raster::DynamicImage, PilError> {
    #[inline]
    fn round_clip(value: f64) -> u8 {
        value.round().clamp(0.0, 255.0) as u8
    }

    match (matrix.len(), target_mode) {
        (4, "RGB") => {
            // Pillow 12.2 retains a legacy four-coefficient RGB-output path:
            // the affine expression is written to the first channel and the
            // other channels are zero for the deterministic public fixture.
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let pixels: Vec<u8> = rgb
                .pixels()
                .flat_map(|p| {
                    [
                        round_clip(
                            matrix[0] * p[0] as f64
                                + matrix[1] * p[1] as f64
                                + matrix[2] * p[2] as f64
                                + matrix[3],
                        ),
                        0,
                        0,
                    ]
                })
                .collect();
            Ok(crate::raster::DynamicImage::ImageRgb8(
                crate::raster::RgbImage::from_raw(w, h, pixels)
                    .ok_or_else(|| PilError::ValueError("matrix conversion failed".into()))?,
            ))
        }
        (4, "L") => {
            // Pillow's four-coefficient matrix path applies one affine
            // expression to the RGB source and produces a luma image.
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let pixels: Vec<u8> = rgb
                .pixels()
                .map(|p| {
                    round_clip(
                        matrix[0] * p[0] as f64
                            + matrix[1] * p[1] as f64
                            + matrix[2] * p[2] as f64
                            + matrix[3],
                    )
                })
                .collect();
            Ok(crate::raster::DynamicImage::ImageLuma8(
                crate::raster::GrayImage::from_raw(w, h, pixels)
                    .ok_or_else(|| PilError::ValueError("matrix conversion failed".into()))?,
            ))
        }
        (12, "RGB") => {
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let pixels: Vec<u8> = rgb
                .pixels()
                .flat_map(|p| {
                    let r = p[0] as f64;
                    let g = p[1] as f64;
                    let b = p[2] as f64;
                    [
                        round_clip(matrix[0] * r + matrix[1] * g + matrix[2] * b + matrix[3]),
                        round_clip(matrix[4] * r + matrix[5] * g + matrix[6] * b + matrix[7]),
                        round_clip(matrix[8] * r + matrix[9] * g + matrix[10] * b + matrix[11]),
                    ]
                })
                .collect();
            Ok(crate::raster::DynamicImage::ImageRgb8(
                crate::raster::RgbImage::from_raw(w, h, pixels)
                    .ok_or_else(|| PilError::ValueError("matrix conversion failed".into()))?,
            ))
        }
        (n, _) => Err(PilError::ValueError(format!(
            "Matrix must be 4 or 12 elements, got {}",
            n
        ))),
    }
}

#[cfg(test)]
mod tests {
    use crate::image::Image;
    use crate::ops::paste::PasteSource;

    #[test]
    fn luma8_to_luma16_conversion_and_paste_preserve_numeric_sample() {
        for mode in ["I;16", "I;16L", "I;16B", "I;16N"] {
            let source = Image::new(1, 1, "L", (17, 17, 17, 255)).expect("source image");
            let converted = source
                .convert(mode, None, None, None, None)
                .expect("I;16 conversion");
            assert_eq!(converted.mode().expect("converted mode"), mode);
            assert_eq!(
                converted.tobytes().expect("converted bytes"),
                match mode {
                    "I;16B" => vec![0, 17],
                    _ => vec![17, 0],
                }
            );

            let mut destination =
                Image::new(1, 1, mode, (0, 0, 0, 255)).expect("destination image");
            destination
                .paste(PasteSource::Image(source), Some((0, 0, 1, 1)), None)
                .expect("I;16 paste");
            assert_eq!(
                destination.tobytes().expect("pasted bytes"),
                match mode {
                    "I;16B" => vec![0, 17],
                    _ => vec![17, 0],
                }
            );
        }
    }
}
