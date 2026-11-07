use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

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
        if let Some(converted) = try_convert_prestandard(self, src_mode, mode, &matrix, dither)? {
            return Ok(converted);
        }

        if mode == Mode::One {
            let dither_enum = parse_dither(dither)?;
            return super::binary::convert_to_binary(self, dither_enum);
        }

        if !(src_mode == Mode::ImageRgb8 && mode == Mode::ImageLuma8)
            && crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
                .scope_for_target(src_mode, mode)
                .is_some()
        {
            if prepared_convert_validates_dither(src_mode, mode) {
                let _ = parse_dither(dither)?;
            }
            return Ok(push_pipeline_convert(self, mode));
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
    Image::push_op(
        image,
        PipelineOp::Convert {
            mode,
            matrix: None,
            dither: None,
        },
    )
}

pub(super) fn prepared_convert_validates_dither(source: Mode, target: Mode) -> bool {
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
    if !has_matrix
        && source_mode == Mode::ImageLaPremultiplied8
        && let Some(composition) = crate::compute::pool_cpu::ops::color::convert::ops::SUPPORT
            .target_composition_for(source_mode, target_mode)
        && composition.kind
            == crate::compute::registry::support::OperationTargetCompositionKind::ConvertGuard
        && let crate::compute::registry::support::OperationRouteOutcome::Rejects {
            reason, kind, ..
        } = composition.outcome
    {
        return Err(kind.into_error(reason));
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
) -> Result<Option<Image>, PilError> {
    // Pillow applies the matrix before its same-mode fast path.
    if let Some(matrix) = matrix {
        return execute_matrix_conversion(image, target_mode, matrix).map(Some);
    }
    if let Some(converted) =
        super::dispatch::try_convert_early_modes(image, source_mode, target_mode, dither)?
    {
        return Ok(Some(converted));
    }
    if target_mode == Mode::ImageLab8 {
        // Pillow's LAB path first normalizes non-RGB-family input to RGBA,
        // then applies its sRGB→LAB transform and attaches the generated
        // output identity profile. Keep both stages explicit in the lazy
        // pipeline so the typed LAB strategy and metadata stay together.
        // NO_PILLOW_SUPPORT: this native normalization also accepts the
        // Rust-only premultiplied-LA source identity.
        let normalized = if matches!(source_mode, Mode::ImageRgb8 | Mode::ImageRgba8 | Mode::ImageRgbx8) {
            image.clone()
        } else {
            image.convert(Mode::ImageRgba8, None, None, None, None)?
        };
        let profile = crate::lab::identity_profile().into();
        return Ok(Some(Image::push_op(
            &normalized,
            PipelineOp::ConvertLab {
                icc_profile: profile,
            },
        )));
    }
    // Preserve the existing eager wrapper for native I;16 destinations.
    // LAB and other declared targets continue to the single prepared Convert
    // operation, where the exact source-mode strategy writes the final mode.
    super::luma16::try_convert_to_luma16(image, source_mode, target_mode)
}

fn execute_matrix_conversion(
    image: &Image,
    target_mode: Mode,
    matrix: &[f64],
) -> Result<Image, PilError> {
    let (materialized, mut operation_receipts) =
        image.materialized_shared_for_ops_with_receipts()?;
    execute_matrix_conversion_from_source(
        image,
        materialized.as_ref(),
        target_mode,
        matrix,
        &mut operation_receipts,
    )
}

fn execute_matrix_conversion_from_source(
    image: &Image,
    materialized: &DynamicImage,
    target_mode: Mode,
    matrix: &[f64],
    operation_receipts: &mut crate::compute::PipelineOperationTelemetryCapture,
) -> Result<Image, PilError> {
    // The LAB matrix extension requires twelve coefficients to produce one
    // RGB sample before the direct LAB kernel runs.
    if !matches!(
        target_mode,
        Mode::ImageLuma8 | Mode::ImageRgb8 | Mode::ImageLab8
    ) {
        return Err(PilError::ValueError(format!(
            "Matrix must be 4 or 12 elements, got {}",
            matrix.len()
        )));
    }
    let operation = PipelineOp::Convert {
        mode: target_mode,
        matrix: Some(matrix.to_vec().into()),
        dither: None,
    };
    let inputs = crate::compute::registry::support::OperationInputs {
        image: materialized,
        palette: crate::compute::registry::support::PaletteInput::default(),
    };
    // Matrix conversion is currently CPU-owned. Preserve an explicit SIMD
    // request and its architecture context; GPU-selected execution keeps that
    // original request while recording the CPU fallback architecture.
    // TODO(pillow-rs#brg0): route an active GPU request through a GPU-owned
    // matrix strategy when that backend declares this operation.
    let (requested_backend, context) = image.cpu_arch_execution_route()?;
    let fallback_reason = (requested_backend == Some(crate::compute::Backend::Gpu)).then(|| {
        format!(
            "GPU matrix Convert has no strategy; used CPU {:?}",
            context.architecture()
        )
    });
    let result = crate::compute::execute_eager_cpu_arch_image_operation_with_context_and_receipts(
        "Convert",
        materialized,
        context,
        requested_backend,
        fallback_reason,
        operation_receipts,
        |context| {
            let prepared = crate::compute::registry::prepare_cpu(&operation, inputs, context)?;
            let selection = *prepared.selection();
            prepared.execute().map(|output| (output, selection))
        },
    )?;
    Image::from_dynamic_with_mode(result, Some(target_mode))
}
