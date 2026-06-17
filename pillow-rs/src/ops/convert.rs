use crate::color;
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{ColorMode, DitherMethod, PipelineOp};
use image::DynamicImage;

/// Parse a mode string into ColorMode.
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

fn parse_dither(s: Option<&str>) -> Option<DitherMethod> {
    match s {
        Some("NONE") | Some("none") => Some(DitherMethod::None),
        Some("FLOYDSTEINBERG") | Some("floydsteinberg") => Some(DitherMethod::FloydSteinberg),
        None => Some(DitherMethod::FloydSteinberg), // PIL default: FloydSteinberg dither
        _ => None,
    }
}

/// Convert image between modes.
/// Supports: L, LA, RGB, RGBA, 1 (bilevel, with/without Floyd-Steinberg dither)
/// Matrix-based conversion: 4-element for single-channel→RGB, 12-element for RGB→RGB color space
impl Image {
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
        if mode == src_mode && src_mode != "P" {
            return Ok(self.copy());
        }
        // P-mode same-mode: PIL defaults to RGB
        let mode = if mode == src_mode && src_mode == "P" {
            "RGB"
        } else {
            mode
        };

        // Matrix-based conversion must be executed immediately since it modifies
        // pixel values directly and can't be represented as a simple mode convert.
        if let Some(mat) = matrix {
            let img = self.materialize()?;
            return convert_with_matrix(&img, mode, &mat)
                .map(|result| Image::Loaded(result, explicit_mode_for(mode)));
        }

        // Handle conversion from non-standard modes (CMYK, HSV, YCbCr, I, F, P).
        // These modes store pixel data in standard DynamicImage containers but with
        // a different interpretation (e.g., CMYK values stored as RGBA). We must
        // materialize first and convert using PIL's exact algorithms.
        if let Some(src_mode) = self.explicit_mode() {
            let target_is_standard = !is_nonstandard_mode(mode);
            if is_nonstandard_mode(src_mode) && target_is_standard {
                // Extract palette before materializing (P-mode palette may be on Pipeline)
                let palette = self.palette();
                let img = self.materialize()?;
                let converted = color::convert_from_nonstandard(src_mode, &img, palette.as_deref())
                    .unwrap_or_else(|| img.to_rgb8().into());
                // If the target is a standard mode, return the converted image directly.
                // For mode "L" etc., derive from the RGB result.
                let result = if mode == "L" || mode == "LA" {
                    if mode == "L" {
                        DynamicImage::ImageLuma8(color::pil_grayscale(&converted))
                    } else {
                        DynamicImage::ImageLumaA8(color::pil_grayscale_alpha(&converted))
                    }
                } else if mode == "RGBA" {
                    DynamicImage::ImageRgba8(converted.to_rgba8())
                } else if mode == "1" {
                    // PIL: convert("1") uses truncated grayscale then threshold at 128
                    let gray = color::pil_grayscale_truncate(&converted);
                    let (w, h) = gray.dimensions();
                    let mut out = image::GrayImage::new(w, h);
                    for (op, gp) in out.pixels_mut().zip(gray.pixels()) {
                        op[0] = if gp[0] >= 128 { 255 } else { 0 };
                    }
                    DynamicImage::ImageLuma8(out)
                } else {
                    converted
                };
                return Ok(Image::Loaded(result, explicit_mode_for(mode)));
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
                    crate::color::cmyk_to_grayscale(&img)
                } else if is_nonstandard_mode(src_mode) {
                    let rgb = crate::color::convert_from_nonstandard(src_mode, &img, None)
                        .unwrap_or_else(|| img.to_rgb8().into());
                    crate::color::pil_grayscale_truncate(&rgb)
                } else {
                    crate::color::pil_grayscale_truncate(&img)
                }
            } else {
                crate::color::pil_grayscale_truncate(&img)
            };
            let (w, h) = gray.dimensions();
            let mut out = image::GrayImage::new(w, h);
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
                    let mut errors = vec![0i32; (w + 1) as usize];
                    let src: Vec<i32> = gray.pixels().map(|p| p[0] as i32).collect();
                    let wu = w as usize;
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
                        // PIL: after the loop, propagate l0 to errors[w] for next row
                        errors[wu] = l0;
                    }
                }
            }
            return Ok(Image::Loaded(
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
            let rgb = img.to_rgb8();
            let (w, h) = rgb.dimensions();
            let rgb_raw = rgb.into_raw();
            let dither = !matches!(dither_enum, Some(DitherMethod::None));
            let (indices, palette_bytes) = web_palette_quantize(&rgb_raw, w, h, dither);
            let mut out = image::GrayImage::new(w, h);
            for (i, pixel) in out.pixels_mut().enumerate() {
                pixel[0] = indices.get(i).copied().unwrap_or(0);
            }
            return Ok(Image::Pipeline {
                source: Arc::new(Image::Loaded(
                    DynamicImage::ImageLuma8(out),
                    Some("P".to_string()),
                )),
                ops: vec![],
                format: None,
                explicit_mode: Some("P".to_string()),
                backend: None,
                palette: Some(palette_bytes),
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
                explicit_mode: ref mut em_field,
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
    img: &image::DynamicImage,
    target_mode: &str,
    matrix: &[f64],
) -> Result<image::DynamicImage, PilError> {
    match (matrix.len(), target_mode) {
        (4, "RGB") => {
            let luma = img.to_luma8();
            let (w, h) = luma.dimensions();
            let pixels: Vec<u8> = luma
                .iter()
                .flat_map(|&l| {
                    let lf = l as f64;
                    [
                        (matrix[0] * lf).clamp(0.0, 255.0) as u8,
                        (matrix[1] * lf).clamp(0.0, 255.0) as u8,
                        (matrix[2] * lf).clamp(0.0, 255.0) as u8,
                    ]
                })
                .collect();
            Ok(image::DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(w, h, pixels).ok_or_else(|| {
                    PilError::ImageError(image::ImageError::from(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "matrix conversion failed",
                    )))
                })?,
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
            Ok(image::DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(w, h, pixels).ok_or_else(|| {
                    PilError::ImageError(image::ImageError::from(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "matrix conversion failed",
                    )))
                })?,
            ))
        }
        (n, _) => Err(PilError::ValueError(format!(
            "Matrix must be 4 or 12 elements, got {}",
            n
        ))),
    }
}
