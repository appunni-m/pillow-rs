//! Pillow-compatible image statistics methods for [`Image`].

mod result;

pub use result::{StatResult, StatValue, stat_from_histogram, stat_from_list};

use crate::compute::registry::support::OperationId;
use crate::compute::PipelineOperationTelemetryCapture;
use crate::error::PilError;

use super::{FormattedExtrema, FormattedPixelValue, Image};

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
    pub fn stat_formatted(&self) -> Result<StatResult, PilError> {
        let bands = self.stat()?;
        Ok(StatResult::from_bands(&bands))
    }

    /// Computes Pillow statistics after applying an optional transparency mask.
    ///
    /// `ImageStat.Stat` is defined in terms of `Image.histogram(mask)`, not an
    /// unmasked scan followed by a validation-only mask check. Reusing the
    /// histogram path keeps the count, extrema, and all derived values
    /// restricted to the non-zero mask pixels.
    pub fn stat_formatted_with_mask(
        &self,
        mask: crate::ops::imageops::ImageOpsMask,
    ) -> Result<StatResult, PilError> {
        match mask {
            crate::ops::imageops::ImageOpsMask::None => self.stat_formatted(),
            crate::ops::imageops::ImageOpsMask::Invalid(type_name) => Err(
                PilError::AttributeError(format!("'{type_name}' object has no attribute 'load'")),
            ),
            crate::ops::imageops::ImageOpsMask::Image(mask) => {
                self.stat_formatted_with_image_mask(&mask)
            }
        }
    }

    fn stat_formatted_with_image_mask(&self, mask: &Image) -> Result<StatResult, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
        operation_receipts.capture();
        let mask_image = mask.materialized_shared()?;
        operation_receipts.capture();
        let prepared_mask = crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::prepare_histogram_mask(
            image.as_ref(),
            Some(mask_image),
        )?;
        let mask_raw = prepared_mask.raw();
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled() {
            // This timing/resource sample covers the terminal scan; ordered
            // source and mask graph operation receipts are restored separately.
            let telemetry = crate::compute::begin_eager_cpu_operation_telemetry(
                "Stat",
                requested_backend,
            );
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
                        telemetry,
                        0,
                        "Stat",
                        selection,
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
    pub fn stat(&self) -> Result<Vec<Vec<f64>>, PilError> {
        let mode = self.mode_identity()?;
        OperationId::Stat.support().for_mode(mode).validate_mode()?;
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
        operation_receipts.capture();
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled() {
            // This timing/resource sample covers the terminal scan; ordered
            // source graph operation receipts are restored separately.
            let telemetry = crate::compute::begin_eager_cpu_operation_telemetry(
                "Stat",
                requested_backend,
            );
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
                        telemetry,
                        0,
                        "Stat",
                        selection,
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
    #[allow(clippy::type_complexity)]
    pub fn getcolors(
        &self,
        maxcolors: u32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
        operation_receipts.capture();
        // Pillow loads before selecting its mode-specific histogram or
        // low-level getcolors path; unsupported-mode errors follow decoding.
        let mode = image.mode();
        OperationId::GetColors.support().for_mode(mode).validate_mode()?;
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled() {
            // This timing/resource sample covers the terminal scan; ordered
            // source graph operation receipts are restored separately.
            let telemetry = crate::compute::begin_eager_cpu_operation_telemetry(
                "GetColors",
                requested_backend,
            );
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
    pub fn getcolors_with_signed_limit(
        &self,
        maxcolors: i32,
    ) -> Result<Option<Vec<(u32, FormattedPixelValue)>>, PilError> {
        if maxcolors >= 0 {
            return self.getcolors(maxcolors as u32);
        }
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
        operation_receipts.capture();
        // The public method loads before Pillow's low-level mode rejection,
        // including when a negative limit guarantees a `None` result.
        let mode = image.mode();
        OperationId::GetColors.support().for_mode(mode).validate_mode()?;
        Ok(None)
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
    pub fn getextrema_formatted(&self) -> Result<FormattedExtrema, PilError> {
        let (width, height) = self.size()?;
        let mode = self.mode_identity()?;
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
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
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
