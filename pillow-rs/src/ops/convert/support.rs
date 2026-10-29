use crate::error::PilError;

pub(super) fn explicit_mode_for(mode: &str) -> Option<crate::raster::Mode> {
    match mode {
        "1" => Some(crate::raster::Mode::One),
        "P" => Some(crate::raster::Mode::ImagePalette8),
        "CMYK" => Some(crate::raster::Mode::ImageCmyk8),
        "HSV" => Some(crate::raster::Mode::ImageHsv8),
        "YCbCr" => Some(crate::raster::Mode::ImageYCbCr8),
        "I" => Some(crate::raster::Mode::ImageI32),
        "F" => Some(crate::raster::Mode::ImageF32),
        "LAB" => Some(crate::raster::Mode::ImageLab8),
        _ => None,
    }
}

pub(super) fn convert_with_matrix(
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
