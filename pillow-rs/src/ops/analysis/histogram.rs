//! Pillow-compatible Histogram wrapper.

use crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::{
    prepare_histogram_mask,
};
use crate::compute::PipelineOperationTelemetryCapture;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::image::Image;
use crate::ops::analysis::HistogramExtrema;

impl Image {
    /// Computes a per-band 256-bin histogram.
    ///
    /// The returned vector is concatenated by band: all 256 bins for band 0,
    /// then all 256 bins for band 1, and so on.
    /// CoreOnly u16 modes map their full sample range to 256 bins using
    /// rounded scaling; CoreOnly f32 modes clamp to `[0, 1]` and round to the
    /// same bins. Both scan native samples without converting the raster.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn histogram(&self) -> Result<Vec<u32>, PilError> {
        self.histogram_with_mask(None)
    }

    /// Computes a native `I`/`F` histogram with caller-supplied extrema.
    pub fn histogram_with_extrema(
        &self,
        extrema: Option<HistogramExtrema>,
    ) -> Result<Vec<u32>, PilError> {
        self.histogram_with_mask_and_extrema(None, extrema)
    }

    /// Computes a per-band histogram restricted to pixels where an optional
    /// mask is non-zero, matching Pillow's masked-histogram semantics.
    pub fn histogram_with_mask(&self, mask: Option<&Image>) -> Result<Vec<u32>, PilError> {
        self.histogram_with_mask_and_extrema(mask, None)
    }

    /// Computes a masked histogram, applying a native `I`/`F` range only when
    /// no mask is present.
    pub fn histogram_with_mask_and_extrema(
        &self,
        mask: Option<&Image>,
        extrema: Option<HistogramExtrema>,
    ) -> Result<Vec<u32>, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
        operation_receipts.capture();
        let mask_image = mask.map(Image::materialized_shared).transpose()?;
        operation_receipts.capture();
        let prepared_mask = prepare_histogram_mask(image.as_ref(), mask_image)?;
        let mask_raw = prepared_mask.raw();
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let extrema = if mask_raw.is_some() {
            None
        } else {
            match (image.mode(), extrema) {
                (crate::raster::Mode::ImageI32, Some(value @ HistogramExtrema::ImageI32 { .. }))
                | (crate::raster::Mode::ImageF32, Some(value @ HistogramExtrema::ImageF32 { .. })) => {
                    Some(value)
                }
                (_, None) => None,
                _ => {
                    return Err(PilError::ValueError(
                        "histogram extrema do not match image mode".into(),
                    ));
                }
            }
        };
        let telemetry = if crate::compute::Backend::pipeline_telemetry_enabled() {
            // TODO(pillow-rs#3ptb): define aggregate timing/resource semantics
            // for source and mask graphs; this summary covers the terminal scan.
            let telemetry = crate::compute::begin_eager_cpu_operation_telemetry(
                "Histogram",
                requested_backend,
            );
            operation_receipts.restore();
            Some(telemetry)
        } else {
            None
        };
        let result: Result<(Vec<u32>, StrategySelection), PilError> = (|| {
            let prepared =
                crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::prepare_histogram_with_extrema(
                    &image,
                    mask_raw,
                    extrema,
                    &context,
                )?;
            let selection = *prepared.selection();
            Ok((prepared.execute()?, selection))
        })();
        if let Some(telemetry) = telemetry {
            match result {
                Ok((histogram, selection)) => {
                    crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                        telemetry,
                        histogram.len().saturating_mul(std::mem::size_of::<u32>()),
                        "Histogram",
                        selection,
                    );
                    Ok(histogram)
                }
                Err(error) => {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                    Err(error)
                }
            }
        } else {
            result.map(|(histogram, _)| histogram)
        }
    }
}
