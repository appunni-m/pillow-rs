use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;

use super::input::parse_dither;
use super::mode::{is_luma16_mode, parse_mode};
use super::support::{convert_with_matrix, explicit_mode_for};

impl Image {
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
        let src_mode = self.mode()?;
        validate_convert_request(&src_mode, mode, matrix.is_some())?;
        if let Some(converted) =
            try_convert_prestandard(self, &src_mode, mode, &matrix, dither, _palette, _colors)?
        {
            return Ok(converted);
        }

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

        if let Some(converted) = super::nonstandard::try_convert_scalar_source(
            self,
            effective_src_mode_name,
            mode,
            matrix.clone(),
            dither,
            _colors,
        )? {
            return Ok(converted);
        }
        if let Some(converted) = super::nonstandard::try_convert_between_nonstandard_modes(
            self,
            effective_src_mode_name,
            mode,
            matrix.clone(),
            dither,
            _colors,
        )? {
            return Ok(converted);
        }
        if let Some(converted) = super::nonstandard::try_convert_to_palette_alpha(
            self,
            effective_src_mode_name,
            mode,
            dither,
        )? {
            return Ok(converted);
        }
        if let Some(converted) =
            super::nonstandard::try_convert_luma16_to_cmyk(self, effective_src_mode_name, mode)?
        {
            return Ok(converted);
        }
        if let Some(converted) =
            super::nonstandard::try_convert_nonstandard_source(self, effective_src_mode_name, mode)?
        {
            return Ok(converted);
        }

        let dither_enum = parse_dither(dither)?;

        if mode == "1" {
            return super::binary::convert_to_binary(self, &effective_src_mode, dither_enum);
        }

        if mode == "P" {
            return super::palette::convert_to_web_palette(self, &src_mode, dither_enum);
        }

        if let Some(converted) =
            super::nonstandard::try_convert_cmyk_to_scalar(self, effective_src_mode_name, mode)?
        {
            return Ok(converted);
        }

        let mode_enum = parse_mode(mode)?;
        if src_mode == "RGB" && mode == "L" {
            // RGB→L and ImageOps.grayscale share Pillow's exact fixed-point
            // luma contract. Reuse its native-layout executor so SIMD can
            // use the packed RGB kernel and GPU can upload RGB/read back L
            // without passing the conversion through an RGBA carrier.
            return Ok(Image::push_op(self, PipelineOp::Grayscale));
        }
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

fn validate_convert_request(
    source_mode: &str,
    target_mode: &str,
    has_matrix: bool,
) -> Result<(), PilError> {
    if source_mode == "La" && !matches!(target_mode, "La" | "LA") && !has_matrix {
        // Only La->LA has a direct converter. All other destinations
        // retry through the L base, which Pillow cannot produce from La.
        return Err(PilError::ValueError(
            "conversion from La to L not supported".into(),
        ));
    }
    // Check the target before source-specific dispatch; PA is handled by its
    // dedicated path below rather than by ColorMode.
    if !matches!(target_mode, "PA" | "RGBX" | "RGBa" | "La" | "LAB") && !is_luma16_mode(target_mode)
    {
        parse_mode(target_mode).map_err(|_| PilError::ValueError("image has wrong mode".into()))?;
    }
    Ok(())
}

fn try_convert_prestandard(
    image: &Image,
    source_mode: &str,
    target_mode: &str,
    matrix: &Option<Vec<f64>>,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    // Core stores bilevel pixels as 0/1 while Pillow's "1" path uses 0/255.
    if source_mode == "1" && target_mode != "1" {
        let materialized = image.materialize()?;
        let mut gray = crate::raster::GrayImage::new(materialized.width(), materialized.height());
        for (output, input) in gray.pixels_mut().zip(materialized.to_luma8().pixels()) {
            output[0] = if input[0] != 0 { 255 } else { 0 };
        }
        let expanded = crate::raster::DynamicImage::ImageLuma8(gray);
        return Ok(Some(Image::from_dynamic(expanded, None).convert(
            target_mode,
            matrix.clone(),
            dither,
            None,
            None,
        )?));
    }

    // Pillow applies the matrix before its same-mode fast path.
    if let Some(matrix) = matrix {
        let materialized = image.materialize()?;
        return convert_with_matrix(&materialized, target_mode, matrix)
            .map(|result| Image::from_dynamic(result, explicit_mode_for(target_mode)))
            .map(Some);
    }
    if let Some(converted) = super::dispatch::try_convert_early_modes(
        image,
        source_mode,
        target_mode,
        dither,
        palette,
        colors,
    )? {
        return Ok(Some(converted));
    }
    if let Some(converted) = super::luma16::try_convert_from_luma16(
        image,
        source_mode,
        target_mode,
        dither,
        palette,
        colors,
    )? {
        return Ok(Some(converted));
    }
    super::luma16::try_convert_to_luma16(image, source_mode, target_mode)
}
