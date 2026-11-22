//! Pillow-compatible image statistics methods for [`Image`].

mod result;

pub use result::{StatResult, StatValue, stat_from_histogram, stat_from_list};

use crate::compute::PipelineOperationTelemetryCapture;
use crate::compute::registry::support::OperationId;
use crate::error::PilError;

use super::{FormattedExtrema, FormattedPixelValue, Image};

const GETCOLORS_MAX_TABLE_LIMIT: i32 = 1_073_741_824;

impl Image {
    // ── Immediate ops (force materialize) ──

    /// Returns Pillow-compatible image statistics in structured form.
    ///
    /// Results use list variants in band order, including single-band images,
    /// matching Pillow's `ImageStat.Stat` attribute contract.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
    pub fn stat_formatted(&self) -> Result<StatResult, PilError> {
        let bands = self.stat()?;
        Ok(StatResult::from_bands(&bands))
    }

    /// Returns formatted statistics using a route captured by the host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn stat_formatted_with_context(
        &self,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<StatResult, PilError> {
        let bands = self.stat_with_execution_context(execution_context)?;
        Ok(StatResult::from_bands(&bands))
    }

    /// Computes Pillow statistics after applying an optional transparency mask.
    ///
    /// `ImageStat.Stat` is defined in terms of `Image.histogram(mask)`, not an
    /// unmasked scan followed by a validation-only mask check. Reusing the
    /// histogram path keeps the count, extrema, and all derived values
    /// restricted to the non-zero mask pixels.
    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
    pub fn stat_formatted_with_mask(
        &self,
        mask: crate::ops::imageops::ImageOpsMask,
    ) -> Result<StatResult, PilError> {
        self.stat_formatted_with_mask_and_context(mask, None)
    }

    /// Computes Pillow statistics with a route captured by the host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn stat_formatted_with_mask_and_context(
        &self,
        mask: crate::ops::imageops::ImageOpsMask,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<StatResult, PilError> {
        match mask {
            crate::ops::imageops::ImageOpsMask::None => {
                self.stat_formatted_with_context(execution_context)
            }
            crate::ops::imageops::ImageOpsMask::Invalid(type_name) => Err(
                PilError::AttributeError(format!("'{type_name}' object has no attribute 'load'")),
            ),
            crate::ops::imageops::ImageOpsMask::Image(mask) => {
                self.stat_formatted_with_image_mask(&mask, execution_context)
            }
        }
    }

    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    fn stat_formatted_with_image_mask(
        &self,
        mask: &Image,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<StatResult, PilError> {
        let routed_image = execution_context.map_or_else(
            || self.clone(),
            |context| self.clone().with_execution_context(context),
        );
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = routed_image.materialized_shared()?;
        operation_receipts.capture();
        let routed_mask = execution_context.map_or_else(
            || mask.clone(),
            |context| mask.clone().with_execution_context(context),
        );
        let mask_image = routed_mask.materialized_shared()?;
        operation_receipts.capture();
        let prepared_mask = crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::prepare_histogram_mask(
            image.as_ref(),
            Some(mask_image),
        )?;
        let mask_raw = prepared_mask.raw();
        let (requested_backend, routed_context) = routed_image.cpu_arch_execution_route_advisory();
        let context = execution_context.unwrap_or(routed_context);
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled() {
            // This timing/resource sample covers the terminal scan; ordered
            // source and mask graph operation receipts are restored separately.
            let telemetry =
                crate::compute::begin_eager_cpu_operation_telemetry("Stat", requested_backend);
            operation_receipts.restore();
            Some(telemetry)
        } else {
            None
        };
        let result = (|| {
            let prepared = crate::compute::pool_cpu::ops::analysis::ops::prepare_stat(
                image.as_ref(),
                mask_raw,
                &context,
            )?;
            let selection = *prepared.selection();
            let statistics = prepared.execute_formatted_histogram()?;
            Ok((StatResult::from_bands(&statistics), selection))
        })();
        if let Some(telemetry) = telemetry {
            match result {
                Ok((statistics, selection)) => {
                    crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                        telemetry, 0, "Stat", selection,
                    );
                    Ok(statistics)
                }
                Err(error) => {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                    Err(error)
                }
            }
        } else {
            result.map(|(statistics, _)| statistics)
        }
    }

    /// Computes per-band statistics.
    ///
    /// The return value is indexed by band. Each band contains:
    /// `[count, sum, sum2, mean, median, rms, var, stddev, min, max]`.
    /// Integer (`"I"`) and floating (`"F"`) modes follow Pillow's histogram
    /// fallback behavior instead of reporting raw numeric-domain statistics.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    pub fn stat(&self) -> Result<Vec<Vec<f64>>, PilError> {
        self.stat_with_execution_context(None)
    }

    /// Computes statistics with a route captured by the host wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn stat_with_execution_context(
        &self,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Vec<Vec<f64>>, PilError> {
        let routed_image = execution_context.map_or_else(
            || self.clone(),
            |context| self.clone().with_execution_context(context),
        );
        let mode = routed_image.mode_identity()?;
        OperationId::Stat.support().for_mode(mode).validate_mode()?;
        let (requested_backend, routed_context) = routed_image.cpu_arch_execution_route_advisory();
        let context = execution_context.unwrap_or(routed_context);
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = routed_image.materialized_shared()?;
        operation_receipts.capture();
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled() {
            // This timing/resource sample covers the terminal scan; ordered
            // source graph operation receipts are restored separately.
            let telemetry =
                crate::compute::begin_eager_cpu_operation_telemetry("Stat", requested_backend);
            operation_receipts.restore();
            Some(telemetry)
        } else {
            None
        };
        let result: Result<
            (
                Vec<Vec<f64>>,
                crate::compute::registry::support::StrategySelection,
            ),
            PilError,
        > = (|| {
            let prepared = crate::compute::pool_cpu::ops::analysis::ops::prepare_stat(
                image.as_ref(),
                None,
                &context,
            )?;
            let selection = *prepared.selection();
            Ok((prepared.execute()?, selection))
        })();
        if let Some(telemetry) = telemetry {
            match result {
                Ok((statistics, selection)) => {
                    crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                        telemetry, 0, "Stat", selection,
                    );
                    Ok(statistics)
                }
                Err(error) => {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                    Err(error)
                }
            }
        } else {
            result.map(|(statistics, _)| statistics)
        }
    }

    /// Returns unique colors and their counts.
    ///
    /// The result is `None` when the image contains more than `maxcolors`
    /// distinct colors. Each color is returned in the scalar or tuple shape
    /// matching the image mode. `I` values retain signed 32-bit precision,
    /// `I;16*` values follow Pillow's wrong-mode error contract, and `F` values
    /// retain their 32-bit floating-point value. CoreOnly multiband modes keep
    /// their native `u16` or `f32` sample values in each returned tuple.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when mode detection or materialization fails.
    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
    #[allow(clippy::type_complexity)]
    pub fn getcolors(
        &self,
        maxcolors: u32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors_with_execution_context(maxcolors, None)
    }

    /// Counts colors using a route captured by the Pillow wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    #[allow(clippy::type_complexity)]
    pub fn getcolors_with_execution_context(
        &self,
        maxcolors: u32,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        let routed_image = execution_context.map_or_else(
            || self.clone(),
            |context| self.clone().with_execution_context(context),
        );
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = routed_image.materialized_shared()?;
        operation_receipts.capture();
        // Pillow loads before selecting its mode-specific histogram or
        // low-level getcolors path; unsupported-mode errors follow decoding.
        let mode = image.mode();
        OperationId::GetColors
            .support()
            .for_mode(mode)
            .validate_mode()?;
        let (requested_backend, routed_context) = routed_image.cpu_arch_execution_route_advisory();
        let context = execution_context.unwrap_or(routed_context);
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled() {
            // This timing/resource sample covers the terminal scan; ordered
            // source graph operation receipts are restored separately.
            let telemetry =
                crate::compute::begin_eager_cpu_operation_telemetry("GetColors", requested_backend);
            operation_receipts.restore();
            Some(telemetry)
        } else {
            None
        };
        let result: Result<_, PilError> = (|| {
            let prepared = crate::compute::pool_cpu::ops::analysis::ops::prepare_getcolors(
                image.as_ref(),
                maxcolors,
                &context,
            )?;
            let selection = *prepared.selection();
            Ok((prepared.execute()?, selection))
        })();
        if let Some(telemetry) = telemetry {
            match result {
                Ok((colors, selection)) => {
                    crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                        telemetry,
                        0,
                        "GetColors",
                        selection,
                    );
                    Ok(colors)
                }
                Err(error) => {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                    Err(error)
                }
            }
        } else {
            result.map(|(colors, _)| colors)
        }
    }

    /// Applies Pillow's signed `maxcolors` argument semantics for bindings.
    /// Negative limits still load and validate the image, then return `None`.
    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
    pub fn getcolors_with_signed_limit(
        &self,
        maxcolors: i32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors_with_signed_limit_and_context(maxcolors, None)
    }

    /// Applies Pillow's signed limit while retaining a caller's route.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn getcolors_with_signed_limit_and_context(
        &self,
        maxcolors: i32,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        if maxcolors >= 0 {
            return self.getcolors_with_execution_context(maxcolors as u32, execution_context);
        }
        let routed_image = execution_context.map_or_else(
            || self.clone(),
            |context| self.clone().with_execution_context(context),
        );
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = routed_image.materialized_shared()?;
        operation_receipts.capture();
        // The public method loads before Pillow's low-level mode rejection,
        // including when a negative limit guarantees a `None` result.
        let mode = image.mode();
        OperationId::GetColors
            .support()
            .for_mode(mode)
            .validate_mode()?;
        Ok(None)
    }

    /// Executes Pillow's public `getcolors` branch selected by visible mode
    /// `1`, `L`, or `P`.
    ///
    /// Those modes use their exact GetColors strategy when the raster has the
    /// same identity. A visible-mode override over another raster uses the
    /// first 256 bins of Histogram, represented as scalar byte values.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    pub fn getcolors_via_histogram_mode(
        &self,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors_via_histogram_mode_with_context(None)
    }

    /// Executes Pillow's histogram-selected route with a captured CPU context.
    // VERIFY: source=reviewed@2026-11-22; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn getcolors_via_histogram_mode_with_context(
        &self,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        let routed_image = execution_context.map_or_else(
            || self.clone(),
            |context| self.clone().with_execution_context(context),
        );
        let mode = routed_image.mode_identity()?;
        if matches!(
            mode,
            crate::raster::Mode::One
                | crate::raster::Mode::ImageLuma8
                | crate::raster::Mode::ImagePalette8
        ) {
            return routed_image.getcolors_with_execution_context(u32::MAX, execution_context);
        }

        let colors = routed_image
            .histogram_with_mask_and_extrema_with_context(None, None, execution_context)?
            .into_iter()
            .take(256)
            .enumerate()
            .filter_map(|(index, count)| {
                (count > 0).then_some((count, FormattedPixelValue::Scalar(index as u8)))
            })
            .collect();
        Ok(Some(colors))
    }

    /// Executes Pillow's low-level `getcolors` branch after its visible mode
    /// selected that route.
    ///
    /// A raster in `1`, `L`, or `P` cannot use ImagingGetColors. Pillow checks
    /// the signed table limit first, so an unrepresentable table size takes
    /// precedence over that wrong-mode error.
    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
    pub fn getcolors_via_low_level_mode_with_signed_limit(
        &self,
        maxcolors: i32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors_via_low_level_mode_with_signed_limit_and_context(maxcolors, None)
    }

    /// Executes the low-level route while retaining a caller's CPU context.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn getcolors_via_low_level_mode_with_signed_limit_and_context(
        &self,
        maxcolors: i32,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        let routed_image = execution_context.map_or_else(
            || self.clone(),
            |context| self.clone().with_execution_context(context),
        );
        let image = routed_image.materialized_shared()?;
        let mode = image.mode();
        let histogram_mode = matches!(
            mode,
            crate::raster::Mode::One
                | crate::raster::Mode::ImageLuma8
                | crate::raster::Mode::ImagePalette8
        );
        let rejected_mode = matches!(
            OperationId::GetColors.support().for_mode(mode),
            crate::compute::registry::support::ModeSupport::Rejected { .. }
        );
        if maxcolors >= GETCOLORS_MAX_TABLE_LIMIT && (histogram_mode || rejected_mode) {
            return Err(PilError::MemoryError(String::new()));
        }
        if histogram_mode {
            return Err(PilError::ValueError("image has wrong mode".into()));
        }
        routed_image.getcolors_with_signed_limit_and_context(maxcolors, execution_context)
    }

    /// Returns `getcolors` with Pillow's scalar/tuple color shape applied.
    ///
    /// Color counting and mode-specific truncation remain in the core; the
    /// binding is responsible only for constructing the host list and tuple
    /// objects.
    pub fn getcolors_formatted(
        &self,
        maxcolors: u32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors(maxcolors)
    }

    /// Returns formatted colors using Pillow's signed `maxcolors` limit.
    pub fn getcolors_formatted_with_signed_limit(
        &self,
        maxcolors: i32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        self.getcolors_with_signed_limit(maxcolors)
    }

    /// Returns extrema using Pillow's one-pair versus per-band result shape.
    // VERIFY: source=reviewed@2026-11-20; parity=pending; benchmark=pending
    pub fn getextrema_formatted(&self) -> Result<FormattedExtrema, PilError> {
        self.getextrema_formatted_with_context(None)
    }

    /// Returns formatted extrema using a route captured by the Pillow wrapper.
    // VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
    #[doc(hidden)]
    pub fn getextrema_formatted_with_context(
        &self,
        execution_context: Option<crate::compute::ExecutionContext>,
    ) -> Result<FormattedExtrema, PilError> {
        let routed_image = execution_context.map_or_else(
            || self.clone(),
            |context| self.clone().with_execution_context(context),
        );
        let (width, height) = routed_image.size()?;
        let mode = routed_image.mode_identity()?;
        if width == 0 || height == 0 {
            // Pillow's ImagingGetExtrema returns its zero-size status before
            // dispatching on sample type, so empty I;16L/B/N images return
            // None even though their nonempty extrema route rejects them.
            let bands = mode.band_count();
            return Ok(if bands == 1 {
                FormattedExtrema::Empty
            } else {
                FormattedExtrema::EmptyMultiple(bands)
            });
        }

        OperationId::GetExtrema
            .support()
            .for_mode(mode)
            .validate_mode()?;
        let (requested_backend, routed_context) = routed_image.cpu_arch_execution_route_advisory();
        let context = execution_context.unwrap_or(routed_context);
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = routed_image.materialized_shared()?;
        operation_receipts.capture();
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled() {
            // This timing/resource sample covers the terminal scan; ordered
            // source graph operation receipts are restored separately.
            let telemetry = crate::compute::begin_eager_cpu_operation_telemetry(
                "GetExtrema",
                requested_backend,
            );
            operation_receipts.restore();
            Some(telemetry)
        } else {
            None
        };
        let result: Result<
            (
                FormattedExtrema,
                crate::compute::registry::support::StrategySelection,
            ),
            PilError,
        > = (|| {
            let prepared = crate::compute::pool_cpu::ops::analysis::ops::prepare_get_extrema(
                image.as_ref(),
                &context,
            )?;
            let selection = *prepared.selection();
            Ok((prepared.execute()?, selection))
        })();
        if let Some(telemetry) = telemetry {
            match result {
                Ok((extrema, selection)) => {
                    crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                        telemetry,
                        0,
                        "GetExtrema",
                        selection,
                    );
                    Ok(extrema)
                }
                Err(error) => {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                    Err(error)
                }
            }
        } else {
            result.map(|(extrema, _)| extrema)
        }
    }
}
