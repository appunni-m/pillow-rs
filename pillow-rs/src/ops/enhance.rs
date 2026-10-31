//! Pillow `ImageEnhance`-style adjustment methods.

use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};
use crate::compute::helpers::enhance::contrast_base;

fn exact_mode(image: &Image) -> Result<Mode, PilError> {
    image.mode_identity()
}

fn uppercase_a_band(mode: Mode) -> Option<usize> {
    match mode {
        Mode::ImageLumaA8 | Mode::ImagePaletteAlpha8 | Mode::ImageLab8 => Some(1),
        Mode::ImageRgba8 => Some(3),
        // NO_PILLOW_SUPPORT: internal high-depth alpha rasters retain their
        // declared alpha sample while constructing a native core base.
        Mode::ImageLumaA16 => Some(1),
        Mode::ImageRgba16 | Mode::ImageRgba32F => Some(3),
        _ => None,
    }
}

fn validate_mode(image: &Image, palette_rejects: bool) -> Result<Mode, PilError> {
    let mode = exact_mode(image)?;
    if mode == Mode::One {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    if palette_rejects && mode == Mode::ImagePalette8 {
        return Err(PilError::ValueError("cannot filter palette images".into()));
    }
    if matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    Ok(mode)
}

impl Image {
    /// Converts a color enhancer's single transparent color through its L base.
    ///
    /// # Errors
    ///
    /// Returns the source pixel coercion, palette, or conversion error.
    pub fn color_transparency(
        &self,
        value: crate::image::PutPixelValue,
    ) -> Result<crate::image::ImageInfoValue, PilError> {
        let mode_identity = exact_mode(self)?;
        // Pillow's RGB integer ink packs R/G/B into the low three bytes.
        // The general scalar pixel API accepts only a native byte, so retain
        // the full integer here before delegating to its component coercion.
        let value = match value {
            crate::image::PutPixelValue::Integer(value) if mode_identity == Mode::ImageRgb8 => {
                crate::image::PutPixelValue::Components(vec![
                    value & 255,
                    (value >> 8) & 255,
                    (value >> 16) & 255,
                ])
            }
            value => value,
        };
        let mut pixel = Image::new(1, 1, mode_identity, (0, 0, 0, 0))?;
        if mode_identity == Mode::ImagePalette8
            && let Some(palette) = self.palette()
        {
            pixel.putpalette(&palette, "RGB")?;
        }
        pixel.putpixel_value(0, 0, value)?;
        let gray = pixel.convert("L", None, None, None, None)?;
        let gray = gray.materialized_shared()?.as_bytes()[0];
        Ok(if mode_identity == Mode::ImageRgb8 {
            crate::image::ImageInfoValue::IntegerTuple(vec![i64::from(gray); 3])
        } else {
            crate::image::ImageInfoValue::Integer(i64::from(gray))
        })
    }

    /// Builds the converted base retained by a Pillow color enhancer.
    ///
    /// L and LA return the same core value; host wrappers preserve public
    /// object identity for those modes. Other modes snapshot an L/LA round trip.
    ///
    /// # Errors
    ///
    /// Returns conversion or materialization failures during construction.
    pub fn color_degenerate(&self) -> Result<Image, PilError> {
        let mode = exact_mode(self)?;
        let pillow_mode = self.mode()?;
        let intermediate = if uppercase_a_band(mode).is_some() { "LA" } else { "L" };
        if matches!(mode, Mode::ImageLuma8 | Mode::ImageLumaA8) {
            return Ok(self.clone());
        }
        if mode == Mode::ImageRgba8 {
            // RGBA -> LA -> RGBA produces the same rounded BT.601 gray in
            // each color channel. The native
            // ColorSaturation(0) path writes that retained base in one pass
            // and keeps RGBA alpha unchanged. Materialize here because this
            // image is the stateful enhancer's construction-time snapshot.
            let mut base = Image::push_op(self, PipelineOp::ColorSaturation { factor: 0.0 });
            base.load()?;
            return Ok(base);
        }
        let mut base = self.convert(intermediate, None, None, None, None)?;
        if mode == Mode::ImageRgbPremultiplied8 {
            return Err(PilError::ValueError(
                "conversion from L to RGBa not supported".into(),
            ));
        }
        if mode == Mode::ImageRgbx8 {
            // RGBX's padding sample becomes 255 through L conversion and
            // participates in the later blend, unlike uppercase alpha.
            let gray = base.materialized_shared()?;
            let bytes = gray
                .as_bytes()
                .iter()
                .flat_map(|&v| [v, v, v, 255])
                .collect();
            let image = crate::image_utils::raw_bytes_to_image_allow_empty(
                gray.width(),
                gray.height(),
                bytes,
                4,
            )?;
            return Ok(Image::from_dynamic_with_mode(image, Some(mode))?);
        }
        base = base.convert(&pillow_mode, None, None, None, None)?;
        base.load()?;
        Ok(base)
    }

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
        let source = self.materialized_shared()?;
        if let Some(base) = contrast_base(&source) {
            // Only the base is observable: avoid an intermediate grayscale
            // frame and a second full-frame conversion of a constant image.
            let mut image = source.clone();
            let output = image
                .as_bytes_mut()
                .ok_or_else(|| PilError::ValueError("image has wrong mode".into()))?;
            for (pixel, original) in output
                .chunks_exact_mut(base.channels)
                .zip(source.as_bytes().chunks_exact(base.channels))
            {
                pixel.copy_from_slice(&base.values[..base.channels]);
                if let Some(alpha) = base.alpha {
                    pixel[alpha] = original[alpha];
                }
            }
            return Ok(Image::from_dynamic_with_mode(image, Some(source.mode()))?);
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
        let mut base = Image::new(gray.width(), gray.height(), crate::raster::Mode::ImageLuma8, (mean, mean, mean, 255))?;
        if source.mode() == Mode::ImageRgbx8 {
            // L -> RGBX fills X with 255; it is a blended sample, not alpha.
            base = Image::new(gray.width(), gray.height(), crate::raster::Mode::ImageRgbx8, (mean, mean, mean, 255))?;
        } else if source.mode() == Mode::ImageRgbPremultiplied8 {
            return Err(PilError::ValueError(
                "conversion from L to RGBa not supported".into(),
            ));
        } else if source.mode() != Mode::ImageLuma8 {
            base = base.convert(&mode, None, None, None, None)?;
        }
        if let Some(alpha) = uppercase_a_band(source.mode()) {
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
            exact_mode(self)?,
            Mode::ImageLuma8
                | Mode::ImageLumaA8
                | Mode::ImageRgb8
                | Mode::ImageRgba8
                | Mode::ImageRgbx8
                | Mode::ImageCmyk8
                | Mode::ImageHsv8
                | Mode::ImageYCbCr8
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
    /// Returns mode or base-conversion errors. Deferred pipeline execution can
    /// report additional materialization failures.
    pub fn enhance_color(&self, factor: f64) -> Result<Image, PilError> {
        if matches!(
            exact_mode(self)?,
            Mode::ImageLuma8
                | Mode::ImageLumaA8
                | Mode::ImageRgb8
                | Mode::ImageRgba8
                | Mode::ImageRgbx8
                | Mode::ImageCmyk8
        ) {
            return Ok(Image::push_op(self, PipelineOp::ColorSaturation { factor }));
        }
        let base = self.color_degenerate()?;
        crate::ops::module_fns::blend(&base, self, factor)
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

#[cfg(test)]
mod tests {
    use crate::Image;

    #[test]
    fn rgb_color_degenerate_matches_convert_roundtrip_across_vector_tails() {
        for (mode, channels, intermediate) in [("RGB", 3usize, "L"), ("RGBA", 4, "LA")] {
            for pixel_count in 1..=17usize {
                let bytes: Vec<u8> = (0..pixel_count * channels)
                    .map(|index| {
                        (index
                            .wrapping_mul(73)
                            .wrapping_add(index / 9 * 11)
                            .wrapping_add(29)) as u8
                    })
                    .collect();
                let source = Image::frombytes(crate::raster::Mode::from_pillow_name(mode).expect("known test mode"), (pixel_count as u32, 1), &bytes)
                    .expect("varied native color source");
                let mut expected = source
                    .convert(intermediate, None, None, None, None)
                    .expect("convert to Pillow's grayscale base")
                    .convert(mode, None, None, None, None)
                    .expect("convert grayscale base to source mode");
                expected.load().expect("materialize reference roundtrip");

                let mut actual = source
                    .color_degenerate()
                    .expect("construct optimized color base");
                actual.load().expect("materialize optimized color base");
                assert_eq!(actual.mode().expect("optimized mode"), mode);
                assert_eq!(
                    actual.tobytes().expect("optimized bytes"),
                    expected.tobytes().expect("reference bytes"),
                    "{mode} with {pixel_count} pixels"
                );
            }
        }
    }
}
