use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::{ColorMode, DitherMethod, PipelineOp};

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

fn parse_dither(s: Option<&str>) -> Option<DitherMethod> {
    match s {
        Some("NONE") | Some("none") => Some(DitherMethod::None),
        Some("FLOYDSTEINBERG") | Some("floydsteinberg") => Some(DitherMethod::FloydSteinberg),
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
        // Matrix-based conversion must be executed immediately since it modifies
        // pixel values directly and can't be represented as a simple mode convert.
        if let Some(mat) = matrix {
            let img = self.materialize()?;
            return convert_with_matrix(&img, mode, &mat).map(|result| Image::Loaded(result, None));
        }

        let mode_enum = parse_mode(mode)?;
        let dither_enum = parse_dither(dither);
        Ok(Image::push_op(
            self,
            PipelineOp::Convert {
                mode: mode_enum,
                matrix: None,
                dither: dither_enum,
            },
        ))
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
                image::RgbImage::from_raw(w, h, pixels)
                    .ok_or_else(|| PilError::ImageError(image::ImageError::from(std::io::Error::new(std::io::ErrorKind::InvalidData, "matrix conversion failed"))))?,
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
                        (matrix[0] * r + matrix[1] * g + matrix[2] * b + matrix[3]).clamp(0.0, 255.0) as u8,
                        (matrix[4] * r + matrix[5] * g + matrix[6] * b + matrix[7]).clamp(0.0, 255.0) as u8,
                        (matrix[8] * r + matrix[9] * g + matrix[10] * b + matrix[11]).clamp(0.0, 255.0) as u8,
                    ]
                })
                .collect();
            Ok(image::DynamicImage::ImageRgb8(
                image::RgbImage::from_raw(w, h, pixels)
                    .ok_or_else(|| PilError::ImageError(image::ImageError::from(std::io::Error::new(std::io::ErrorKind::InvalidData, "matrix conversion failed"))))?,
            ))
        }
        (n, _) => Err(PilError::ValueError(format!(
            "Matrix must be 4 or 12 elements, got {}", n
        ))),
    }
}
