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
    matches!(mode, "CMYK" | "HSV" | "YCbCr" | "I" | "F" | "P")
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

fn parse_dither(s: Option<&str>) -> Option<DitherMethod> {
    match s {
        Some("NONE") | Some("none") => Some(DitherMethod::None),
        Some("FLOYDSTEINBERG") | Some("floydsteinberg") => Some(DitherMethod::FloydSteinberg),
        None => Some(DitherMethod::FloydSteinberg), // PIL default: FloydSteinberg dither
        _ => None,
    }
}

/// Pillow-compatible image mode conversion methods.
impl Image {
    /// Converts this image to another Pillow mode.
    ///
    /// # Inputs
    ///
    /// - `mode`: destination Pillow mode, such as `"L"`, `"RGB"`, `"RGBA"`,
    ///   `"CMYK"`, `"HSV"`, `"YCbCr"`, `"I"`, `"F"`, `"P"`, or `"1"`.
    /// - `matrix`: optional Pillow conversion matrix for immediate conversion.
    ///   Four values convert single-channel input to RGB-family output; twelve
    ///   values convert RGB input through a 3x4 color matrix.
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
        if let Some(src_mode) = self.explicit_mode() {
            let target_is_standard = !is_nonstandard_mode(mode);
            // Non-standard sources must be materialized and converted to RGB
            // before reaching a standard target OR a CMYK target (Pillow's
            // CMYK inverse runs on the RGB values).  CMYK->CMYK is identity.
            if is_nonstandard_mode(src_mode) && (target_is_standard || mode == "CMYK") {
                // Extract palette before materializing (P-mode palette may be on Pipeline)
                let palette = self.palette();
                let img = self.materialize()?;
                let converted = color::convert_from_nonstandard(src_mode, &img, palette.as_deref())
                    .unwrap_or_else(|| img.to_rgb8().into());
                // For mode "L" etc., derive from the RGB result.
                let result = if mode == "CMYK" {
                    // Apply the RGB inverse directly on the converted RGB.
                    DynamicImage::ImageRgba8(crate::color::rgb_to_cmyk_inverse(
                        &converted.to_rgb8(),
                    ))
                } else if mode == "L" || mode == "LA" {
                    if mode == "L" && src_mode == "YCbCr" {
                        // Pillow's C converter maps YCbCr to L through the Y
                        // band directly, not through the RGB luma.
                        DynamicImage::ImageLuma8(ycbcr_luma8(&img))
                    } else if mode == "L" {
                        DynamicImage::ImageLuma8(color::pil_grayscale(&converted)?)
                    } else {
                        DynamicImage::ImageLumaA8(color::pil_grayscale_alpha(&converted)?)
                    }
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

        let dither_enum = parse_dither(dither);

        // Special case: converting to binary mode "1" — must eagerly execute
        // because the pipeline's scalar::convert doesn't handle binary threshold/dither.
        if mode == "1" {
            let img = self.materialize()?;
            // Use truncated grayscale (PIL uses integer truncation, not rounding)
            let gray = if let Some(src_mode) = self.explicit_mode() {
                if src_mode == "CMYK" {
                    crate::color::cmyk_to_grayscale(&img)?
                } else if is_nonstandard_mode(src_mode) {
                    let rgb = crate::color::convert_from_nonstandard(src_mode, &img, None)
                        .unwrap_or_else(|| img.to_rgb8().into());
                    crate::color::pil_grayscale_truncate(&rgb)?
                } else {
                    crate::color::pil_grayscale_truncate(&img)?
                }
            } else {
                crate::color::pil_grayscale_truncate(&img)?
            };
            let (w, h) = gray.dimensions();
            let mut out = crate::raster::GrayImage::new(w, h);
            match dither_enum {
                Some(DitherMethod::None) => {
                    // Threshold at 128 (PIL: pixel >= 128 -> 255, else 0)
                    for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                        op[0] = if gp[0] >= 128 { 255 } else { 0 };
                    }
                }
                _ => {
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
                    let source_is_nonstandard = self
                        .explicit_mode()
                        .is_some_and(|mode| is_nonstandard_mode(mode));
                    let is_rgb = matches!(img.color(), crate::raster::ColorType::Rgb8)
                        && !source_is_nonstandard;
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
            // Pillow's four-coefficient RGB matrix path applies one affine
            // channel expression and leaves the remaining channels at zero.
            // Treat the source as RGB here; reducing it to luma loses the
            // matrix offset and diverges for the accepted RGB->RGB form.
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
