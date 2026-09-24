//! Pillow `ImageEnhance`-style adjustment methods.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::DynamicImage;

pub(crate) struct ContrastBase {
    pub(crate) mean: u8,
    pub(crate) channels: usize,
    pub(crate) values: [u8; 4],
    pub(crate) alpha: Option<usize>,
}

/// Native-mode equivalent of convert-to-L, rounded mean, and L-to-source base.
pub(crate) fn contrast_base(img: &DynamicImage, mode: Option<&str>) -> Option<ContrastBase> {
    let (channels, alpha, cmyk) = match (img, mode) {
        (DynamicImage::ImageLuma8(_), None | Some("L")) => (1, None, false),
        (DynamicImage::ImageLumaA8(_), None | Some("LA")) => (2, Some(1), false),
        (DynamicImage::ImageRgb8(_), None | Some("RGB")) => (3, None, false),
        (DynamicImage::ImageRgba8(_), None | Some("RGBA")) => (4, Some(3), false),
        (DynamicImage::ImageRgba8(_), Some("RGBX")) => (4, None, false),
        (DynamicImage::ImageRgba8(_), Some("CMYK")) => (4, None, true),
        _ => return None,
    };
    let source = img.as_bytes();
    let count = source.len() / channels;
    let sum: u64 = source
        .chunks_exact(channels)
        .map(|pixel| {
            let value = if channels <= 2 {
                pixel[0]
            } else if cmyk {
                let nk = 255 - u32::from(pixel[3]);
                let rgb = |c: u8| (nk - crate::color::muldiv255(u32::from(c), nk)) as u8;
                crate::color::rgb_to_luma_u8(rgb(pixel[0]), rgb(pixel[1]), rgb(pixel[2]))
            } else {
                crate::color::rgb_to_luma_u8(pixel[0], pixel[1], pixel[2])
            };
            u64::from(value)
        })
        .sum();
    let mean = if count == 0 {
        0
    } else {
        (sum as f64 / count as f64 + 0.5) as u8
    };
    Some(ContrastBase {
        mean,
        channels,
        alpha,
        values: if cmyk {
            [0, 0, 0, 255 - mean]
        } else {
            [mean, mean, mean, 255]
        },
    })
}

pub(crate) fn contrast_lut(base: &ContrastBase, factor: f64) -> Vec<u8> {
    (0..base.channels)
        .flat_map(|band| {
            (0..=255u8).map(move |value| {
                if base.alpha == Some(band) {
                    return value;
                }
                let midpoint = f32::from(base.values[band]);
                (factor as f32)
                    .mul_add(f32::from(value) - midpoint, midpoint)
                    .clamp(0.0, 255.0) as u8
            })
        })
        .collect()
}

fn validate_mode(image: &Image, palette_rejects: bool) -> Result<(), PilError> {
    let mode = image.mode()?;
    if mode == "1" {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    if palette_rejects && mode == "P" {
        return Err(PilError::ValueError("cannot filter palette images".into()));
    }
    if matches!(mode.as_str(), "P" | "PA") {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    Ok(())
}

impl Image {
    /// Adjusts brightness by `factor`.
    ///
    /// `1.0` is unchanged and `0.0` produces black.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn enhance_brightness(&self, factor: f64) -> Result<Image, PilError> {
        validate_mode(self, false)?;
        Ok(Image::push_op(self, PipelineOp::Brightness { factor }))
    }

    /// Builds the base image retained by a Pillow contrast enhancer.
    ///
    /// The mean is rounded after conversion to L. Converting that constant
    /// image back to the source mode defines the per-band base, and uppercase
    /// alpha is copied from this image at construction time. Later source
    /// mutations must not alter this snapshot.
    ///
    /// # Errors
    ///
    /// Returns the source/destination conversion error or a materialization
    /// error. Modes rejected by blending can still construct a base image.
    pub fn contrast_degenerate(&self) -> Result<Image, PilError> {
        let mode = self.mode()?;
        if matches!(mode.as_str(), "L" | "LA" | "RGB" | "RGBA" | "RGBX" | "CMYK") {
            let source = self.materialized_shared()?;
            if let Some(base) = contrast_base(&source, Some(&mode)) {
                // Only the base is observable: avoid an intermediate grayscale
                // frame and a second full-frame conversion of a constant image.
                let mut bytes = vec![base.values[0]; source.as_bytes().len()];
                if let Some(alpha) = base.alpha {
                    for (pixel, original) in bytes
                        .chunks_exact_mut(base.channels)
                        .zip(source.as_bytes().chunks_exact(base.channels))
                    {
                        pixel[alpha] = original[alpha];
                    }
                } else if base.channels == 4 {
                    for pixel in bytes.chunks_exact_mut(4) {
                        pixel[3] = base.values[3];
                    }
                }
                let image = crate::image_utils::raw_bytes_to_image_allow_empty(
                    source.width(),
                    source.height(),
                    bytes,
                    base.channels,
                )?;
                return Ok(Image::from_dynamic(
                    image,
                    matches!(mode.as_str(), "RGBX" | "CMYK").then_some(mode),
                ));
            }
        }

        let gray = self.convert("L", None, None, None, None)?;
        let gray = gray.materialized_shared()?;
        let samples = gray.as_bytes();
        let mean = if samples.is_empty() {
            0
        } else {
            let sum: u64 = samples.iter().map(|&value| u64::from(value)).sum();
            (sum as f64 / samples.len() as f64 + 0.5) as u8
        };
        let mut base = Image::new(gray.width(), gray.height(), "L", (mean, mean, mean, 255))?;
        if mode == "RGBX" {
            // L -> RGBX fills X with 255; it is a blended sample, not alpha.
            base = Image::new(gray.width(), gray.height(), "RGBX", (mean, mean, mean, 255))?;
        } else if mode == "RGBa" {
            return Err(PilError::ValueError(
                "conversion from L to RGBa not supported".into(),
            ));
        } else if mode != "L" {
            base = base.convert(&mode, None, None, None, None)?;
        }
        if let Some(alpha) = self.getbands()?.iter().position(|band| band == "A") {
            base.putalpha_data(&self.getchannel(alpha as i32)?)?;
        }
        // Pillow creates this image during __init__, including conversion
        // errors. Materialize now so enhancement only blends the saved base.
        base.load()?;
        Ok(base)
    }

    /// Adjusts contrast by `factor`.
    ///
    /// `1.0` is unchanged and `0.0` produces a solid gray image.
    ///
    /// # Errors
    ///
    /// Returns mode or conversion errors for unsupported layouts. Supported
    /// byte modes may defer base construction and execution until materialization.
    pub fn enhance_contrast(&self, factor: f64) -> Result<Image, PilError> {
        if matches!(
            self.mode()?.as_str(),
            "L" | "LA" | "RGB" | "RGBA" | "RGBX" | "CMYK" | "HSV" | "YCbCr"
        ) {
            // The one-shot core API retains its lazy graph so a later backend
            // choice also governs any pending source operations. Stateful host
            // enhancers separately retain contrast_degenerate() at construction.
            return Ok(Image::push_op(self, PipelineOp::Contrast { factor }));
        }
        let degenerate = self.contrast_degenerate()?;
        crate::ops::module_fns::blend(&degenerate, self, factor)
    }

    /// Adjusts color saturation by `factor`.
    ///
    /// `1.0` is unchanged and `0.0` produces grayscale.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn enhance_color(&self, factor: f64) -> Result<Image, PilError> {
        validate_mode(self, false)?;
        Ok(Image::push_op(self, PipelineOp::ColorSaturation { factor }))
    }

    /// Adjusts sharpness by `factor`.
    ///
    /// `1.0` is unchanged, values below `1.0` blur, and values above `1.0`
    /// sharpen.
    ///
    /// # Errors
    ///
    /// Currently returns `Ok(Image)`; deferred pipeline execution reports later
    /// materialization failures.
    pub fn enhance_sharpness(&self, factor: f64) -> Result<Image, PilError> {
        validate_mode(self, true)?;
        Ok(Image::push_op(self, PipelineOp::Sharpness { factor }))
    }
}
