use crate::color;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{ColorMode, DitherMethod, PipelineOp};
use crate::raster::DynamicImage;

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
    match img.pending_palette_transparency()? {
        crate::image::PaletteTransparency::Index(index) => {
            let mut table = vec![255u8; 256];
            table[usize::from(index)] = 0;
            Some(table)
        }
        crate::image::PaletteTransparency::Table(alpha) => Some(alpha),
    }
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
    /// A host image object, which Pillow treats as the palette argument.
    Image,
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
            PythonConvertPaletteInput::None | PythonConvertPaletteInput::Image => None,
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
        if mode != "PA" {
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
        if let Some(src_mode) = Some(effective_src_mode.as_str()) {
            let target_is_standard = !is_nonstandard_mode(mode);
            // Non-standard sources must be materialized and converted to RGB
            // before reaching a standard target OR a CMYK target (Pillow's
            // CMYK inverse runs on the RGB values).  CMYK->CMYK is identity.
            if is_nonstandard_mode(src_mode)
                && (target_is_standard || mode == "CMYK" || (mode == "PA" && src_mode == "P"))
            {
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
                    if matches!(src_mode, "I" | "F") {
                        // Pillow's Convert.c sends I/F sources through the
                        // grayscale-to-CMYK path, not the RGB inverse: C=M=Y=0
                        // and K=255-gray.  The old Rust path inverted the
                        // broadcast RGB representation and diverged for these
                        // source modes.
                        let gray = color::pil_grayscale(&converted)?;
                        let (w, h) = gray.dimensions();
                        let mut cmyk = crate::raster::RgbaImage::new(w, h);
                        for (out, input) in cmyk.pixels_mut().zip(gray.pixels()) {
                            *out = crate::raster::Rgba([0, 0, 0, 255 - input[0]]);
                        }
                        DynamicImage::ImageRgba8(cmyk)
                    } else {
                        // Apply the RGB inverse directly on the converted RGB.
                        DynamicImage::ImageRgba8(crate::color::rgb_to_cmyk_inverse(
                            &converted.to_rgb8(),
                        ))
                    }
                } else if mode == "L" || mode == "LA" {
                    if mode == "L" && src_mode == "YCbCr" {
                        // Pillow's C converter maps YCbCr to L through the Y
                        // band directly, not through the RGB luma.
                        DynamicImage::ImageLuma8(ycbcr_luma8(&img))
                    } else if mode == "L" {
                        DynamicImage::ImageLuma8(color::pil_grayscale(&converted)?)
                    } else {
                        let mut la = color::pil_grayscale_alpha(&converted)?;
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
                } else if mode == "PA" && src_mode == "P" {
                    // Pillow P->PA keeps the palette indices with the palette
                    // alpha band (opaque unless a transparency marks entries).
                    let indices = img.to_luma8();
                    let (w, h) = indices.dimensions();
                    let mut pa = crate::raster::GrayAlphaImage::new(w, h);
                    let table = palette_alpha_for_convert(self);
                    for (op, ip) in pa.pixels_mut().zip(indices.pixels()) {
                        op[0] = ip[0];
                        op[1] = table
                            .as_ref()
                            .and_then(|t| t.get(usize::from(ip[0])))
                            .copied()
                            .unwrap_or(255);
                    }
                    let loaded = crate::image::Image::Loaded(crate::image::LoadedData {
                        image: std::sync::Arc::new(crate::raster::DynamicImage::ImageLumaA8(pa)),
                        explicit_mode: Some("PA".to_owned()),
                        decoded_mode: crate::raster::ColorType::La8.into(),
                        palette: palette.map(|p| p.to_vec()),
                        palette_alpha: self.palette_alpha(),
                        source_format: None,
                        info: None,
                    });
                    return Ok(loaded);
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
        }

        if mode == "PA" && src_mode == "L" {
            // Pillow's Convert.c represents L->PA as the luma sample plus an
            // opaque alpha byte. This path is used by Image.paste when a
            // luma image is pasted into a palette-alpha destination.
            let luma = self.materialize()?.to_luma8();
            let (width, height) = luma.dimensions();
            let pa = crate::raster::GrayAlphaImage::from_fn(width, height, |x, y| {
                crate::raster::LumaA([luma.get_pixel(x, y)[0], 255])
            });
            return Ok(Image::from_dynamic(
                DynamicImage::ImageLumaA8(pa),
                Some("PA".to_owned()),
            ));
        }

        let dither_enum = parse_dither(dither)?;

        // Special case: converting to binary mode "1" — must eagerly execute
        // because the pipeline's scalar::convert doesn't handle binary threshold/dither.
        if mode == "1" {
            let img = self.materialize()?;
            // Use truncated grayscale (PIL uses integer truncation, not rounding)
            let gray = if effective_src_mode == "CMYK" {
                crate::color::cmyk_to_grayscale(&img)?
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
            use crate::ops::quantize::web_palette_quantize;
            use std::sync::Arc;

            let img = self.materialize()?;
            let (w, h) = (img.width(), img.height());
            let (indices, palette_bytes) = if matches!(src_mode.as_str(), "1" | "L") {
                // Pillow 12.2 Convert.c maps L/1 samples directly to P indices
                // and installs the identity grayscale palette. Web quantization
                // would change the indices before mixed-mode paste/composite.
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
            return Ok(Image::Pipeline {
                source: Arc::new(Image::from_dynamic(
                    DynamicImage::ImageLuma8(out),
                    Some("P".to_string()),
                )),
                ops: vec![],
                format: None,
                explicit_mode: Some("P".to_string()),
                backend: None,
                palette: Some(palette_bytes),
                palette_alpha: None,
                materialized: crate::image::materialization_cache(),
            });
        }

        let mode_enum = parse_mode(mode)?;
        let mut result = Image::push_op(
            self,
            PipelineOp::Convert {
                mode: mode_enum,
                matrix: None,
                dither: dither_enum,
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
                        (matrix[0] * p[0] as f64
                            + matrix[1] * p[1] as f64
                            + matrix[2] * p[2] as f64
                            + matrix[3])
                            .clamp(0.0, 255.0) as u8,
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
                    (matrix[0] * p[0] as f64
                        + matrix[1] * p[1] as f64
                        + matrix[2] * p[2] as f64
                        + matrix[3])
                        .clamp(0.0, 255.0) as u8
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
                        (matrix[0] * r + matrix[1] * g + matrix[2] * b + matrix[3])
                            .clamp(0.0, 255.0) as u8,
                        (matrix[4] * r + matrix[5] * g + matrix[6] * b + matrix[7])
                            .clamp(0.0, 255.0) as u8,
                        (matrix[8] * r + matrix[9] * g + matrix[10] * b + matrix[11])
                            .clamp(0.0, 255.0) as u8,
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
