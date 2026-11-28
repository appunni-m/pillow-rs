//! Pipeline evaluation and indexed-palette execution strategies.

use std::sync::Arc;

use crate::error::PilError;
use crate::image::{Image, PaletteData, PipelinePrefixCache};
use crate::pipeline::PipelineOp;
use crate::raster::{DynamicImage, Mode};

use super::palette_output::{evaluate_palette_target_convert, retain_preserved_palette_metadata};
use super::{PipelinePaletteOutput, expand_palette, expand_palette_alpha};

impl Image {
    /// Evaluates a pipeline without publishing its result into the pipeline's
    /// ordinary materialization cache.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(super) fn evaluate_pipeline(
        source: &Image,
        ops: &[PipelineOp],
        output_mode: &Option<Mode>,
        backend: Option<crate::compute::Backend>,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
        palette_data: &Option<PaletteData>,
        preserves_palette: bool,
        prefix_cache: Option<&PipelinePrefixCache>,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<(Arc<DynamicImage>, Option<PipelinePaletteOutput>), PilError> {
        if ops.is_empty() {
            let image =
                super::retag_declared_palette_mode(source.materialized_shared()?, *output_mode);
            return Self::validate_declared_pipeline_mode(image, *output_mode)
                .map(|image| (image, None));
        }
        let mut palette_output = None;
        let result = if let Some(prefix_cache) =
            prefix_cache.filter(|cache| cache.ops_len > 0 && cache.ops_len < ops.len())
        {
            let prefix_ops = &ops[..prefix_cache.ops_len];
            let prefix_image = prefix_cache
                .materialized
                .get_or_init(|| {
                    let prepared = crate::compute::prepare_execution_with_context(
                        prefix_ops,
                        backend,
                        execution_context,
                    )?;
                    let source_image = source.materialized_shared()?;
                    let mut prefix_palette_output = None;
                    Self::evaluate_pipeline_with_image(
                        source,
                        source_image.as_ref(),
                        &source_image,
                        prefix_ops,
                        output_mode,
                        &prepared,
                        palette,
                        palette_alpha,
                        palette_data,
                        &mut prefix_palette_output,
                        preserves_palette,
                    )
                })
                .clone()?;
            let suffix_ops = &ops[prefix_cache.ops_len..];
            let prepared = crate::compute::prepare_execution_with_context(
                suffix_ops,
                backend,
                execution_context,
            )?;
            let resolved_palette = palette.clone().or_else(|| source.extract_palette());
            let resolved_alpha = palette_alpha
                .clone()
                .or_else(|| source.palette_alpha())
                .unwrap_or_default();
            let inputs = crate::compute::registry::support::PaletteInput {
                rgb: resolved_palette.as_deref(),
                alpha: &resolved_alpha,
                data: palette_data.as_ref().or_else(|| source.palette_data()),
            };
            crate::compute::execute_prepared_shared_with_palette(
                &prepared,
                suffix_ops,
                &prefix_image,
                *output_mode,
                inputs,
            )?
        } else {
            let prepared =
                crate::compute::prepare_execution_with_context(ops, backend, execution_context)?;
            let source_image = source.materialized_shared()?;
            Self::evaluate_pipeline_with_image(
                source,
                source_image.as_ref(),
                &source_image,
                ops,
                output_mode,
                &prepared,
                palette,
                palette_alpha,
                palette_data,
                &mut palette_output,
                preserves_palette,
            )?
        };
        Self::validate_declared_pipeline_mode(result, *output_mode)
            .map(|image| (image, palette_output))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub(super) fn evaluate_pipeline_uncached(
        source: &Image,
        ops: &[PipelineOp],
        output_mode: &Option<Mode>,
        backend: Option<crate::compute::Backend>,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
        palette_data: &Option<PaletteData>,
        preserves_palette: bool,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<(Arc<DynamicImage>, Option<PipelinePaletteOutput>), PilError> {
        if ops.is_empty() {
            let image = super::retag_declared_palette_mode(
                Arc::new(source.materialize_uncached()?),
                *output_mode,
            );
            return Self::validate_declared_pipeline_mode(image, *output_mode)
                .map(|image| (image, None));
        }
        let prepared =
            crate::compute::prepare_execution_with_context(ops, backend, execution_context)?;
        let (source_pixels, source_palette_output) = source.materialize_uncached_with_metadata()?;
        let source_image = Arc::new(source_pixels);
        let resolved_palette = palette.clone().or_else(|| {
            source_palette_output
                .as_ref()
                .and_then(|output| output.palette.clone())
        });
        let resolved_palette = resolved_palette.or_else(|| source.extract_palette());
        let resolved_alpha = palette_alpha.clone().or_else(|| {
            source_palette_output
                .as_ref()
                .and_then(|output| output.palette_alpha.clone())
        });
        let resolved_alpha = resolved_alpha.or_else(|| source.palette_alpha());
        let resolved_palette_data = palette_data.clone().or_else(|| {
            source_palette_output
                .as_ref()
                .and_then(|output| output.palette_data.clone())
        });
        let mut palette_output = None;
        let result = Self::evaluate_pipeline_with_image(
            source,
            source_image.as_ref(),
            &source_image,
            ops,
            output_mode,
            &prepared,
            &resolved_palette,
            &resolved_alpha,
            &resolved_palette_data,
            &mut palette_output,
            preserves_palette,
        )?;
        Self::validate_declared_pipeline_mode(result, *output_mode)
            .map(|image| (image, palette_output))
    }

    /// Keeps the producer's exact raster identity aligned with declared
    /// pipeline metadata without reinterpreting its samples.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn validate_declared_pipeline_mode(
        result: Arc<DynamicImage>,
        declared_mode: Option<Mode>,
    ) -> Result<Arc<DynamicImage>, PilError> {
        if let Some(expected) = declared_mode {
            let actual = result.mode();
            if actual != expected {
                return Err(PilError::InternalError(format!(
                    "pipeline output mode mismatch: declared {expected:?}, produced {actual:?}"
                )));
            }
        }
        Ok(result)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn evaluate_palette_cpu_operation(
        source: &Image,
        img: &DynamicImage,
        ops: &[PipelineOp],
        prepared: &crate::compute::PreparedExecution,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
        palette_data: &Option<PaletteData>,
    ) -> Result<Option<Arc<DynamicImage>>, PilError> {
        if !source.has_palette_samples()
            || ops.len() != 1
            || prepared.context().backend() != crate::compute::Backend::Cpu
        {
            return Ok(None);
        }
        // TODO(pillow-rs#3ptb): move indexed Equalize expansion into the GPU
        // mode strategy when GPU execution adopts the typed palette contract.
        let palette_strategy = match (source.known_mode_identity(), ops.first()) {
            (_, Some(PipelineOp::Grayscale)) => true,
            (Some(Mode::ImagePaletteAlpha8), Some(PipelineOp::Convert { mode, .. }))
                if !matches!(mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8) =>
            {
                // PA's second sample has destination-specific conversion
                // behavior. Keep Convert on the PA strategy so RGBX and RGBa
                // retain their distinct fourth-lane contracts instead of
                // normalizing through RGBA first.
                true
            }
            (
                Some(Mode::ImagePalette8 | Mode::ImagePaletteAlpha8),
                Some(PipelineOp::Equalize | PipelineOp::EqualizeMasked { .. }),
            ) => true,
            _ => false,
        };
        if !palette_strategy {
            return Ok(None);
        }

        let palette_rgb = palette.clone().or_else(|| source.extract_palette());
        let palette_alpha = palette_alpha
            .clone()
            .or_else(|| source.palette_alpha())
            .unwrap_or_default();
        let input = crate::compute::registry::support::OperationInputs {
            image: img,
            palette: crate::compute::registry::support::PaletteInput {
                rgb: palette_rgb.as_deref(),
                alpha: &palette_alpha,
                data: palette_data.as_ref().or_else(|| source.palette_data()),
            },
        };
        let result = crate::compute::execute_prepared_cpu_arch_operation(prepared, &ops[0], input)?;
        Ok(Some(Arc::new(result)))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn evaluate_palette_pipeline(
        source: &Image,
        img: &DynamicImage,
        ops: &[PipelineOp],
        output_mode: &Option<Mode>,
        prepared: &crate::compute::PreparedExecution,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
        palette_data: &Option<PaletteData>,
    ) -> Result<Option<Arc<DynamicImage>>, PilError> {
        if !source.has_palette_samples() {
            return Ok(None);
        }
        let source_mode = img.mode();
        if prepared.context().backend() == crate::compute::Backend::Cpu
            && matches!(source_mode, Mode::ImagePalette8 | Mode::ImagePaletteAlpha8)
        {
            // Public Image::push_op can bypass the Pillow wrapper's mode
            // admission. Check the exact indexed source declaration before
            // this adapter expands its samples and could dispatch as RGB(A).
            // Unsafe indexed operations are isolated at a pipeline boundary,
            // so later operations are prepared against their actual result.
            if let Some(op) = ops.first() {
                op.support().for_mode(source_mode).validate_mode()?;
            }
        }
        let all_safe = ops
            .iter()
            .all(|op| Self::is_palette_safe_op_for_mode(source_mode, op));
        if all_safe {
            let palette_mode = if *output_mode == Some(Mode::ImagePaletteAlpha8)
                || source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8)
            {
                Mode::ImagePaletteAlpha8
            } else {
                Mode::ImagePalette8
            };
            let resolved_palette = palette.clone().or_else(|| source.extract_palette());
            let resolved_alpha = palette_alpha
                .clone()
                .or_else(|| source.palette_alpha())
                .unwrap_or_default();
            let inputs = crate::compute::registry::support::PaletteInput {
                rgb: resolved_palette.as_deref(),
                alpha: &resolved_alpha,
                data: palette_data.as_ref().or_else(|| source.palette_data()),
            };
            let result = crate::compute::execute_prepared_with_palette(
                prepared,
                ops,
                img,
                Some(palette_mode),
                inputs,
            )?;
            return Ok(Some(Arc::new(result)));
        }

        let palette_mode = if source.known_mode_identity() == Some(Mode::ImagePaletteAlpha8) {
            Mode::ImagePaletteAlpha8
        } else {
            Mode::ImagePalette8
        };

        // PA always expands through a palette before color-dependent
        // operations, even when no palette is attached. Pillow's
        // libImaging/Convert.c indexed path treats an empty PA palette
        // as all black; falling through to the raw LA bytes would
        // incorrectly expose the index as grayscale (for example, index
        // 7 would become RGB(7, 7, 7)).
        let palette = palette.clone().or_else(|| source.extract_palette());
        let pillow_output_mode = output_mode
            .as_ref()
            .copied()
            .filter(|mode| mode.is_pillow_mode());
        if palette_mode == Mode::ImagePaletteAlpha8 {
            let expanded = expand_palette_alpha(
                &img.to_luma_alpha8(),
                palette.as_deref().unwrap_or_default(),
            );
            let result =
                crate::compute::execute_prepared(prepared, ops, &expanded, pillow_output_mode)?;
            return Ok(Some(Arc::new(result)));
        }
        if let Some(palette) = palette {
            let palette_alpha = palette_alpha
                .clone()
                .or_else(|| source.palette_alpha())
                .unwrap_or_default();
            let expanded = expand_palette(&img.to_luma8(), &palette, &palette_alpha);
            let result =
                crate::compute::execute_prepared(prepared, ops, &expanded, pillow_output_mode)?;
            return Ok(Some(Arc::new(result)));
        }
        Ok(None)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn compatibility_execution_mode(
        source: &Image,
        img: &DynamicImage,
        ops: &[PipelineOp],
        output_mode: &Option<Mode>,
    ) -> Result<Option<Mode>, PilError> {
        // RemapPalette needs the source mode to distinguish L samples from P
        // indices; its pipeline result may stay P-tagged.
        let remap_source_mode =
            if ops.len() == 1 && matches!(ops.first(), Some(PipelineOp::RemapPalette { .. })) {
                Some(source.mode_identity()?)
            } else {
                None
            };
        // A one-step conversion also needs the source tag. RGBX and RGBa share
        // storage but have different fourth-byte semantics.
        let convert_source_mode =
            if ops.len() == 1 && matches!(ops.first(), Some(PipelineOp::Convert { .. })) {
                Some(source.mode_identity()?)
            } else {
                None
            };
        // Encoded I;16 input uses generic Luma16 storage but retains its exact
        // source identity in the lazy image header.
        let source_luma16_mode = if remap_source_mode.is_none()
            && convert_source_mode.is_none()
            && output_mode.is_none()
            && matches!(img, DynamicImage::ImageI16(_))
        {
            Some(source.mode_identity()?)
        } else {
            None
        };
        // Flattened pipelines start from the root buffer; an explicit output
        // mode must not reinterpret setup operations such as PutPixel.
        let initial_source_mode = if remap_source_mode.is_none()
            && convert_source_mode.is_none()
            && source_luma16_mode.is_none()
        {
            Some(source.mode_identity()?)
        } else {
            None
        };
        let logical_mode = remap_source_mode
            .or(convert_source_mode)
            .or(source_luma16_mode)
            .or(initial_source_mode)
            .or(*output_mode);
        Ok(logical_mode)
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn evaluate_pipeline_with_image(
        source: &Image,
        img: &DynamicImage,
        shared_img: &Arc<DynamicImage>,
        ops: &[PipelineOp],
        output_mode: &Option<Mode>,
        prepared: &crate::compute::PreparedExecution,
        palette: &Option<Vec<u8>>,
        palette_alpha: &Option<Vec<u8>>,
        palette_data: &Option<PaletteData>,
        palette_output: &mut Option<PipelinePaletteOutput>,
        preserves_palette: bool,
    ) -> Result<Arc<DynamicImage>, PilError> {
        if let Some((image, output)) = evaluate_palette_target_convert(
            source,
            img,
            ops,
            prepared,
            palette,
            palette_alpha,
            palette_data,
        )? {
            *palette_output = Some(output);
            return Ok(image);
        }
        if let Some(result) = Self::evaluate_palette_cpu_operation(
            source,
            img,
            ops,
            prepared,
            palette,
            palette_alpha,
            palette_data,
        )? {
            *palette_output = retain_preserved_palette_metadata(
                source,
                &result,
                palette,
                palette_alpha,
                palette_data,
                preserves_palette,
            );
            return Ok(result);
        }
        if let Some(result) = Self::evaluate_palette_pipeline(
            source,
            img,
            ops,
            output_mode,
            prepared,
            palette,
            palette_alpha,
            palette_data,
        )? {
            *palette_output = retain_preserved_palette_metadata(
                source,
                &result,
                palette,
                palette_alpha,
                palette_data,
                preserves_palette,
            );
            return Ok(result);
        }
        // RGBX carries padding rather than alpha; preserve its exact source
        // identity instead of normalizing pixels to RGBA.
        let execution_mode = Self::compatibility_execution_mode(source, img, ops, output_mode)?;
        let resolved_palette = if palette.is_none() {
            source.extract_palette()
        } else {
            None
        };
        let resolved_alpha = if palette_alpha.is_none() {
            source.palette_alpha()
        } else {
            None
        };
        let inputs = crate::compute::registry::support::PaletteInput {
            rgb: palette.as_deref().or(resolved_palette.as_deref()),
            alpha: palette_alpha
                .as_deref()
                .or(resolved_alpha.as_deref())
                .unwrap_or_default(),
            data: palette_data.as_ref().or_else(|| source.palette_data()),
        };
        let result = crate::compute::execute_prepared_shared_with_palette(
            prepared,
            ops,
            shared_img,
            execution_mode,
            inputs,
        )?;
        *palette_output = retain_preserved_palette_metadata(
            source,
            &result,
            palette,
            palette_alpha,
            palette_data,
            preserves_palette,
        );
        Ok(result)
    }
}
