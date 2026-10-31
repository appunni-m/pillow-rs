use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

use super::input::parse_dither;
use super::mode::{is_convert_target_mode, is_luma16_mode, is_nonstandard_mode};

impl Image {
    /// Converts this image to another Pillow mode.
    ///
    /// # Inputs
    ///
    /// - `mode`: exact destination raster mode.
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
        mode: Mode,
        matrix: Option<Vec<f64>>,
        dither: Option<&str>,
        _palette: Option<&str>,
        _colors: Option<u32>,
    ) -> Result<Image, PilError> {
        let src_mode = self.mode_identity()?;
        validate_convert_request(src_mode, mode, matrix.is_some())?;
        if matches!(src_mode, Mode::ImageI32 | Mode::ImageF32)
            && mode == Mode::ImageRgbx8
            && let Some(converted) =
                super::dispatch::try_convert_scalar_to_rgbx(self, src_mode)?
        {
            return Ok(converted);
        }
        if let Some(converted) =
            try_convert_prestandard(self, src_mode, mode, &matrix, dither, _palette, _colors)?
        {
            return Ok(converted);
        }

        if matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
            && crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
                .for_mode(src_mode)
                .scope_for_target(mode)
                .is_some()
        {
            return super::palette::convert_prepared_palette(self, src_mode, mode, dither);
        }

        if mode == Mode::One {
            let dither_enum = parse_dither(dither)?;
            return super::binary::convert_to_binary(self, dither_enum);
        }

        if !(src_mode == Mode::ImageRgb8 && mode == Mode::ImageLuma8)
            && crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
                .for_mode(src_mode)
                .scope_for_target(mode)
                .is_some()
        {
            if prepared_convert_validates_dither(src_mode, mode) {
                let _ = parse_dither(dither)?;
            }
            return Ok(push_pipeline_convert(self, mode));
        }

        if let Some(converted) = super::nonstandard::try_convert_between_nonstandard_modes(
            self,
            src_mode,
            mode,
            matrix.clone(),
            dither,
            _colors,
        )? {
            return Ok(converted);
        }
        if let Some(converted) =
            super::nonstandard::try_convert_nonstandard_source(self, src_mode, mode)?
        {
            return Ok(converted);
        }

        let _ = parse_dither(dither)?;

        if src_mode == Mode::ImageRgb8 && mode == Mode::ImageLuma8 {
            // RGB→L and ImageOps.grayscale share Pillow's exact fixed-point
            // luma contract. Reuse its native-layout executor so SIMD can
            // use the packed RGB kernel and GPU can upload RGB/read back L
            // without passing the conversion through an RGBA carrier.
            return Ok(Image::push_op(self, PipelineOp::Grayscale));
        }
        Ok(push_pipeline_convert(self, mode))
    }
}

fn push_pipeline_convert(image: &Image, mode: Mode) -> Image {
    let mut result = Image::push_op(
        image,
        PipelineOp::Convert {
            mode,
            matrix: None,
            dither: None,
        },
    );
    result.set_pipeline_output_mode(Some(mode));
    result
}

fn prepared_convert_validates_dither(source: Mode, target: Mode) -> bool {
    // The prior One→L route widened to L, whose same-mode conversion returned
    // before parsing dither. Preserve that validation behavior on the direct
    // One strategy.
    if source == Mode::One && target == Mode::ImageLuma8 {
        return false;
    }
    // The previous I;16→I/F routes queued numeric conversion directly and did
    // not parse a dither value; native u16 strategies keep that contract.
    if is_luma16_mode(source) && matches!(target, Mode::ImageI32 | Mode::ImageF32) {
        return false;
    }
    if matches!(source, Mode::ImageI32 | Mode::ImageF32) {
        return matches!(
            target,
            Mode::ImageLumaA8
                | Mode::ImageRgba8
                | Mode::ImageCmyk8
                | Mode::ImageYCbCr8
                | Mode::ImageHsv8
        );
    }
    if is_nonstandard_mode(source) {
        return target != source
            && !(source == Mode::ImageCmyk8
                && matches!(target, Mode::ImageI32 | Mode::ImageF32))
            && matches!(
                target,
                Mode::ImageHsv8
                    | Mode::ImageYCbCr8
                    | Mode::ImagePalette8
                    | Mode::ImageI32
                    | Mode::ImageF32
            );
    }
    true
}

fn validate_convert_request(
    source_mode: Mode,
    target_mode: Mode,
    has_matrix: bool,
) -> Result<(), PilError> {
    // Internal high-depth rasters have no Pillow conversion target spelling,
    // but exact same-mode Rust conversion is a typed identity copy.
    if !has_matrix && source_mode == target_mode {
        return Ok(());
    }
    if source_mode == Mode::ImageLaPremultiplied8
        && !matches!(target_mode, Mode::ImageLaPremultiplied8 | Mode::ImageLumaA8)
        && !has_matrix
    {
        // Only La->LA has a direct converter. All other destinations
        // retry through the L base, which Pillow cannot produce from La.
        return Err(PilError::ValueError(
            "conversion from La to L not supported".into(),
        ));
    }
    // Check the target before source-specific dispatch; PA is handled by its
    // dedicated path below rather than by ColorMode.
    if !is_convert_target_mode(target_mode) {
        return Err(PilError::ValueError("image has wrong mode".into()));
    }
    Ok(())
}

fn try_convert_prestandard(
    image: &Image,
    source_mode: Mode,
    target_mode: Mode,
    matrix: &Option<Vec<f64>>,
    dither: Option<&str>,
    palette: Option<&str>,
    colors: Option<u32>,
) -> Result<Option<Image>, PilError> {
    // Preserve the archived matrix path's L normalization for bilevel data.
    // Ordinary conversions use the native One strategy, which expands each
    // stored nonzero sample directly into the destination's native layout.
    if source_mode == Mode::One && matrix.is_some() {
        // Preserve Pillow's archived bilevel normalization before selecting
        // the prepared matrix strategy for its resulting L source.
        let materialized = image.materialize()?;
        let mut gray = crate::raster::GrayImage::new(materialized.width(), materialized.height());
        for (output, input) in gray.pixels_mut().zip(materialized.to_luma8().pixels()) {
            output[0] = if input[0] != 0 { 255 } else { 0 };
        }
        let expanded = crate::raster::DynamicImage::ImageLuma8(gray);
        return Ok(Some(Image::from_dynamic(expanded).convert(
            target_mode,
            matrix.clone(),
            dither,
            None,
            None,
        )?));
    }

    // Pillow applies the matrix before its same-mode fast path.
    if let Some(matrix) = matrix {
        return execute_matrix_conversion(image, target_mode, matrix).map(Some);
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
    // Convert::SUPPORT declares this source-to-LAB composition and its child
    // ConvertLab operation; this wrapper builds the corresponding lazy graph.
    super::luma16::try_convert_to_luma16(image, source_mode, target_mode)
}

fn execute_matrix_conversion(
    image: &Image,
    target_mode: Mode,
    matrix: &[f64],
) -> Result<Image, PilError> {
    let materialized = image.materialize()?;
    if !matches!(target_mode, Mode::ImageLuma8 | Mode::ImageRgb8) {
        return Err(PilError::ValueError(format!(
            "Matrix must be 4 or 12 elements, got {}",
            matrix.len()
        )));
    }
    let operation = PipelineOp::Convert {
        mode: target_mode,
        matrix: Some(matrix.to_vec()),
        dither: None,
    };
    let inputs = crate::compute::registry::support::OperationInputs {
        image: &materialized,
        palette: crate::compute::registry::support::PaletteInput::default(),
    };
    let selected_backend = crate::compute::preferred_active_backend()?
        .map(|(backend, _)| backend)
        .unwrap_or(crate::compute::Backend::Cpu);
    // Matrix conversion is currently CPU-owned. Preserve an explicit SIMD
    // request so its architecture selection and Default fallback are recorded;
    // GPU matrix execution can be added with the GPU operation contract.
    // TODO(pillow-rs#brg0): route an active GPU request through a GPU-owned
    // matrix strategy when that backend declares this operation.
    let requested_backend = match selected_backend {
        crate::compute::Backend::Gpu => crate::compute::Backend::Cpu,
        backend => backend,
    };
    let result = crate::compute::execute_eager_cpu_arch_image_operation(
        "Convert",
        &materialized,
        requested_backend,
        |context| {
            let prepared = crate::compute::registry::prepare_cpu(&operation, inputs, context)?;
            let selection = *prepared.selection();
            prepared.execute().map(|output| (output, selection))
        },
    )?;
    Image::from_dynamic_with_mode(result, Some(target_mode))
}
