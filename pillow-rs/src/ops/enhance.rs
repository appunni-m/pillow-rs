//! Pillow `ImageEnhance`-style adjustment methods.

use crate::compute::helpers::enhance::{contrast_base, contrast_base_hidden_lanes};
use crate::compute::registry::support::{ModeSupport, OperationId};
use crate::error::PilError;
use crate::image::Image;
use crate::pipeline::PipelineOp;
use crate::raster::Mode;

fn mode_support(operation: OperationId, mode: Mode) -> ModeSupport {
    operation.support().for_mode(mode)
}

/// Keeps a source pipeline's explicit backend on a newly built fallback graph.
fn inherit_pipeline_backend(source: &Image, output: Image) -> Image {
    match source.backend() {
        Some(backend) => output.use_backend(backend),
        None => output,
    }
}

fn exact_mode(image: &Image) -> Result<Mode, PilError> {
    image.mode_identity()
}

fn validate_operation_mode(image: &Image, operation: OperationId) -> Result<Mode, PilError> {
    let mode = exact_mode(image)?;
    match mode_support(operation, mode) {
        ModeSupport::Supported { .. } => Ok(mode),
        ModeSupport::Rejected { reason, kind, .. } => Err(kind.into_error(reason)),
    }
}

fn has_prepared_mode_strategy(operation: OperationId, mode: Mode) -> bool {
    operation
        .support()
        .route_for_mode(mode)
        .has_prepared_strategy()
}

impl Image {
    /// Converts a color enhancer's single transparent color through its L base.
    ///
    /// # Errors
    ///
    /// Returns the source pixel coercion, palette, or conversion error.
    // TODO(pillow-rs#3ptb): verify mode output and selected strategy/fallback
    // receipts for the captured context across the frozen CPU parity matrix.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn color_transparency(
        &self,
        value: crate::image::PutPixelValue,
    ) -> Result<crate::image::ImageInfoValue, PilError> {
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        self.color_transparency_with_execution_route(value, requested_backend, context)
    }

    /// Converts Color's transparent color under its parent execution route.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn color_transparency_with_execution_route(
        &self,
        value: crate::image::PutPixelValue,
        requested_backend: Option<crate::compute::Backend>,
        context: crate::compute::ExecutionContext,
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
        pixel.putpixel_value_with_context(0, 0, value, requested_backend, context)?;
        let gray = pixel.convert_with_execution_context(Mode::ImageLuma8, None, None, context)?;
        // Preserve the route captured before constructing the temporary image;
        // that wrapper has no knowledge of its caller's selected architecture.
        let gray = inherit_pipeline_backend(self, gray).with_execution_context(context);
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
    // TODO(pillow-rs#3ptb): verify the converted base and construction-time
    // fault order across the frozen CPU mode/architecture matrix.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn color_degenerate(&self) -> Result<Image, PilError> {
        let (_, context) = self.cpu_arch_execution_route_advisory();
        self.color_degenerate_with_execution_context(context)
    }

    /// Builds Color's retained base under a route captured by its host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn color_degenerate_with_execution_context(
        &self,
        context: crate::compute::ExecutionContext,
    ) -> Result<Image, PilError> {
        let routed = self.clone().with_execution_context(context);
        routed
            .color_degenerate_with_context(context)
            .map(|image| image.with_execution_context(context))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn color_degenerate_with_context(
        &self,
        context: crate::compute::ExecutionContext,
    ) -> Result<Image, PilError> {
        let mode = exact_mode(self)?;
        let route = OperationId::ColorSaturation.support().route_for_mode(mode);
        let intermediate = route.intermediate_mode().ok_or_else(|| {
            PilError::InternalError(
                "ColorSaturation support omitted its degenerate intermediate mode".into(),
            )
        })?;
        if matches!(mode, Mode::ImageLuma8 | Mode::ImageLumaA8) {
            return Ok(self.clone());
        }
        if mode == Mode::ImageRgba8 {
            // RGBA -> LA -> RGBA produces the same rounded BT.601 gray in
            // each color channel. The native
            // ColorSaturation(0) path writes that retained base in one pass
            // and keeps RGBA alpha unchanged. Materialize here because this
            // image is the stateful enhancer's construction-time snapshot.
            let mut base = Image::push_op(self, PipelineOp::ColorSaturation { factor: 0.0 })
                .with_execution_context(context);
            base.load()?;
            return Ok(base);
        }
        let mut conversion_source = self.clone().with_execution_context(context);
        let mut base = match conversion_source.convert_with_execution_context(
            intermediate,
            None,
            None,
            context,
        ) {
            Ok(base) => base.with_execution_context(context),
            Err(error) => {
                // Pillow's first Image.convert loads its source before route
                // validation, including a rejected La -> L conversion.
                conversion_source.load()?;
                return Err(error);
            }
        };
        if mode == Mode::ImageRgbPremultiplied8 {
            // Pillow completes RGBa -> L before attempting the unsupported
            // L -> RGBa conversion. Materialize the first route so decode or
            // conversion faults keep their observable precedence.
            base.load()?;
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
        // The declared composed ColorSaturation routes (YCbCr, LAB, and HSV)
        // append both Convert children before this eager load. The pipeline is
        // prepared and executed as one batch, so its operations share one
        // Backend/Architecture context while retaining the construction-time
        // snapshot required by the stateful enhancer.
        base = match base.convert_with_execution_context(mode, None, None, context) {
            Ok(base) => base.with_execution_context(context),
            Err(error) => {
                // Complete the first Convert before exposing a synchronous
                // admission error from Pillow's second sequential Convert.
                base.load()?;
                return Err(error);
            }
        };
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
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn enhance_brightness(&self, factor: f64) -> Result<Image, PilError> {
        validate_operation_mode(self, OperationId::Brightness)?;
        Ok(Image::push_op(self, PipelineOp::Brightness { factor }))
    }

    /// Builds the base image retained by a Pillow contrast enhancer.
    ///
    /// The mean is rounded after conversion to L. Converting that constant
    /// image back to the source mode defines the per-band base. Pillow's
    /// uppercase `A` band is copied at construction time, including LAB's
    /// color-space component. Later source mutations must not alter this
    /// snapshot.
    ///
    /// # Errors
    ///
    /// Returns the source/destination conversion error or a materialization
    /// error. Modes rejected by blending can still construct a base image.
    // TODO(pillow-rs#3ptb): verify base pixels, alpha, error order, and context
    // receipts across the frozen CPU mode/architecture matrix.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn contrast_degenerate(&self) -> Result<Image, PilError> {
        let (_, context) = self.cpu_arch_execution_route_advisory();
        self.contrast_degenerate_with_execution_context(context)
    }

    /// Builds Contrast's retained base under a route captured by its host wrapper.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn contrast_degenerate_with_execution_context(
        &self,
        context: crate::compute::ExecutionContext,
    ) -> Result<Image, PilError> {
        let routed = self.clone().with_execution_context(context);
        routed
            .contrast_degenerate_with_context(context)
            .map(|image| image.with_execution_context(context))
    }

    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    fn contrast_degenerate_with_context(
        &self,
        context: crate::compute::ExecutionContext,
    ) -> Result<Image, PilError> {
        let mode = exact_mode(self)?;
        let routed = self.clone().with_execution_context(context);
        let source = routed.materialized_shared()?;
        if let Some(base) = contrast_base(&source) {
            let source_mode = source.mode();
            // Only the degenerate base image is needed: avoid an intermediate
            // grayscale frame and a second full-frame conversion of a constant image.
            let mut image = source.as_ref().clone();
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
            if source_mode.hidden_pixel_lane_count().is_some() {
                let pixel_count = (image.width() as usize)
                    .checked_mul(image.height() as usize)
                    .ok_or_else(|| {
                        PilError::DimensionError("Contrast base lane extent overflow".into())
                    })?;
                let lanes = contrast_base_hidden_lanes(source_mode, &base, pixel_count)
                    .ok_or_else(|| {
                        PilError::InternalError(
                            "Contrast base hidden-lane layout was invalid".into(),
                        )
                    })?;
                if !image.replace_hidden_pixel_lanes(Some(lanes)) {
                    return Err(PilError::InternalError(
                        "Contrast base lanes did not match its mode".into(),
                    ));
                }
            }
            // TODO(pillow-rs#3ptb): verify the resulting compact-mode bytes
            // through same-pixel-size Paste in the frozen CPU parity matrix.
            return Ok(Image::from_dynamic_with_mode(image, Some(source_mode))?);
        }

        let gray = self
            .convert_with_execution_context(Mode::ImageLuma8, None, None, context)?
            .with_execution_context(context);
        let gray = gray.materialized_shared()?;
        let samples = gray.as_bytes();
        let mean = if samples.is_empty() {
            0
        } else {
            let sum: u64 = samples.iter().map(|&value| u64::from(value)).sum();
            (sum as f64 / samples.len() as f64 + 0.5) as u8
        };
        let mut base = Image::new(
            gray.width(),
            gray.height(),
            crate::raster::Mode::ImageLuma8,
            (mean, mean, mean, 255),
        )?;
        if source.mode() == Mode::ImageRgbx8 {
            // L -> RGBX fills X with 255; it is a blended sample, not alpha.
            base = Image::new(
                gray.width(),
                gray.height(),
                crate::raster::Mode::ImageRgbx8,
                (mean, mean, mean, 255),
            )?;
        } else if source.mode() == Mode::ImageRgbPremultiplied8 {
            return Err(PilError::ValueError(
                "conversion from L to RGBa not supported".into(),
            ));
        } else if source.mode() != Mode::ImageLuma8 {
            base = base.convert_with_execution_context(mode, None, None, context)?;
            base = inherit_pipeline_backend(self, base).with_execution_context(context);
        }
        if let Some(band) = source.mode().uppercase_a_band_index() {
            let channel = self.getchannel_with_context(band as i32, context)?;
            base.putalpha_data(&channel)?;
            base = inherit_pipeline_backend(self, base).with_execution_context(context);
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
    // TODO(pillow-rs#3ptb): verify fallback Blend pixels, errors, and context
    // receipts for every frozen mode and CPU architecture. The base children
    // and queued Blend graph already receive the captured context.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn enhance_contrast(&self, factor: f64) -> Result<Image, PilError> {
        let mode = exact_mode(self)?;
        if has_prepared_mode_strategy(OperationId::Contrast, mode) {
            // The one-shot core API retains its lazy graph so a later backend
            // choice also governs any pending source operations. Stateful host
            // enhancers separately retain contrast_degenerate() at construction.
            return Ok(Image::push_op(self, PipelineOp::Contrast { factor }));
        }
        let (_, context) = self.cpu_arch_execution_route_advisory();
        let degenerate = self.contrast_degenerate_with_context(context)?;
        let blended = crate::ops::module_fns::blend(&degenerate, self, factor)?;
        Ok(inherit_pipeline_backend(self, blended).with_execution_context(context))
    }

    /// Adjusts contrast using the architecture captured by the host wrapper.
    // TODO(pillow-rs#3ptb): verify fallback Blend pixels, faults, and route
    // receipts for each frozen CPU mode/architecture case.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn enhance_contrast_with_execution_context(
        &self,
        factor: f64,
        context: crate::compute::ExecutionContext,
    ) -> Result<Image, PilError> {
        let mode = exact_mode(self)?;
        let source = self.clone().with_execution_context(context);
        if has_prepared_mode_strategy(OperationId::Contrast, mode) {
            return Ok(Image::push_op(&source, PipelineOp::Contrast { factor })
                .with_execution_context(context));
        }
        let degenerate = source.contrast_degenerate_with_context(context)?;
        let blended = crate::ops::module_fns::blend(&degenerate, &source, factor)?;
        Ok(inherit_pipeline_backend(&source, blended).with_execution_context(context))
    }

    /// Adjusts color saturation by `factor`.
    ///
    /// `1.0` is unchanged and `0.0` produces grayscale.
    ///
    /// # Errors
    ///
    /// Returns mode or base-conversion errors. Deferred pipeline execution can
    /// report additional materialization failures.
    // TODO(pillow-rs#3ptb): verify fallback Blend pixels, errors, and context
    // receipts for every frozen mode and CPU architecture. The base children
    // and queued Blend graph already receive the captured context.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn enhance_color(&self, factor: f64) -> Result<Image, PilError> {
        let mode = exact_mode(self)?;
        if has_prepared_mode_strategy(OperationId::ColorSaturation, mode) {
            return Ok(Image::push_op(self, PipelineOp::ColorSaturation { factor }));
        }
        let (_, context) = self.cpu_arch_execution_route_advisory();
        let base = self.color_degenerate_with_context(context)?;
        let blended = crate::ops::module_fns::blend(&base, self, factor)?;
        Ok(inherit_pipeline_backend(self, blended).with_execution_context(context))
    }

    /// Adjusts color saturation using the architecture captured by the host wrapper.
    // TODO(pillow-rs#3ptb): verify fallback Blend pixels, faults, and route
    // receipts for each frozen CPU mode/architecture case.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn enhance_color_with_execution_context(
        &self,
        factor: f64,
        context: crate::compute::ExecutionContext,
    ) -> Result<Image, PilError> {
        let mode = exact_mode(self)?;
        let source = self.clone().with_execution_context(context);
        if has_prepared_mode_strategy(OperationId::ColorSaturation, mode) {
            return Ok(
                Image::push_op(&source, PipelineOp::ColorSaturation { factor })
                    .with_execution_context(context),
            );
        }
        let base = source.color_degenerate_with_context(context)?;
        let blended = crate::ops::module_fns::blend(&base, &source, factor)?;
        Ok(inherit_pipeline_backend(&source, blended).with_execution_context(context))
    }

    /// Checks direct Rust Sharpness mode admission without executing pixels.
    ///
    /// The Python enhancer uses public Filter and Blend calls to preserve
    /// Pillow's load and constructor-snapshot boundaries; this query is not
    /// used to preflight that host sequence.
    ///
    /// # Errors
    ///
    /// Returns the declared rejection for a mode unsupported by Rust Sharpness.
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn preflight_enhance_sharpness(&self) -> Result<(), PilError> {
        let mode = exact_mode(self)?;
        if mode == Mode::ImagePalette8 {
            mode_support(OperationId::Sharpness, mode).validate_mode()?;
        }
        Ok(())
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
    // VERIFY: source=reviewed@2026-11-23; parity=pending; benchmark=pending
    pub fn enhance_sharpness(&self, factor: f64) -> Result<Image, PilError> {
        validate_operation_mode(self, OperationId::Sharpness)?;
        Ok(Image::push_op(self, PipelineOp::Sharpness { factor }))
    }
}
