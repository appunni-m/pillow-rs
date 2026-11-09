//! Pillow-compatible entropy methods for [`Image`].

use crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::{
    prepare_entropy_histogram, prepare_entropy_histogram_with_extrema, prepare_entropy_mask,
};
use crate::compute::registry::support::StrategySelection;
use crate::compute::{Backend, PipelineOperationTelemetryCapture};
use crate::error::PilError;
use crate::ops::analysis::HistogramExtrema;

use super::Image;

// NO_PILLOW_SUPPORT: CoreOnly high-depth modes use the same native sample
// Histogram extension as their exact-mode Histogram implementation.

impl Image {
    /// Returns Shannon entropy using Pillow-compatible per-band histograms.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn entropy(&self) -> Result<f64, PilError> {
        self.entropy_with_mask(None)
    }

    /// Computes entropy with an optional native `I`/`F` histogram range.
    // NO_PILLOW_SUPPORT: Pillow's public `Image.entropy` accepts a mask only.
    pub fn entropy_with_extrema(&self, extrema: Option<HistogramExtrema>) -> Result<f64, PilError> {
        self.entropy_with_mask_and_extrema(None, extrema)
    }

    /// Computes entropy over pixels where an optional mask is non-zero,
    /// matching Pillow's masked-entropy semantics.
    pub fn entropy_with_mask(&self, mask: Option<&Image>) -> Result<f64, PilError> {
        self.entropy_with_mask_and_extrema(mask, None)
    }

    /// Computes entropy from the selected native histogram, ignoring extrema
    /// when a mask is present.
    pub fn entropy_with_mask_and_extrema(
        &self,
        mask: Option<&Image>,
        extrema: Option<HistogramExtrema>,
    ) -> Result<f64, PilError> {
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
        operation_receipts.capture();
        let mask_image = mask.map(Image::materialized_shared).transpose()?;
        operation_receipts.capture();
        let prepared_mask = prepare_entropy_mask(image.as_ref(), mask_image)?;
        let mask_raw = prepared_mask.raw();
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let extrema = if mask_raw.is_some() {
            None
        } else {
            match (image.mode(), extrema) {
                (
                    crate::raster::Mode::ImageI32,
                    Some(value @ HistogramExtrema::ImageI32 { .. }),
                )
                | (
                    crate::raster::Mode::ImageF32,
                    Some(value @ HistogramExtrema::ImageF32 { .. }),
                ) => Some(value),
                (_, None) => None,
                _ => {
                    return Err(PilError::ValueError(
                        "histogram extrema do not match image mode".into(),
                    ));
                }
            }
        };
        let telemetry = if Backend::pipeline_telemetry_enabled() {
            // This timing/resource sample covers the terminal scan; ordered
            // source and mask graph operation receipts are restored separately.
            let telemetry =
                crate::compute::begin_eager_cpu_operation_telemetry("Entropy", requested_backend);
            operation_receipts.restore();
            Some(telemetry)
        } else {
            None
        };
        let result: Result<(f64, StrategySelection), PilError> = (|| {
            let (prepared, selection) = if let Some(extrema) = extrema {
                prepare_entropy_histogram_with_extrema(&image, mask_raw, Some(extrema), &context)?
            } else {
                prepare_entropy_histogram(&image, mask_raw, &context)?
            };
            let bins = prepared.execute()?;
            Ok((entropy_from_bins(&bins), selection))
        })();
        if let Some(telemetry) = telemetry {
            match result {
                Ok((entropy, selection)) => {
                    crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                        telemetry,
                        std::mem::size_of::<f64>(),
                        "Entropy",
                        selection,
                    );
                    Ok(entropy)
                }
                Err(error) => {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                    Err(error)
                }
            }
        } else {
            result.map(|(entropy, _)| entropy)
        }
    }
}

fn entropy_from_bins(bins: &[u32]) -> f64 {
    let total = bins.iter().copied().sum::<u32>() as f64;
    let mut entropy = 0.0f64;
    for &count in bins {
        if count > 0 {
            let probability = f64::from(count) / total;
            // Pillow 12.2.0 `_entropy` accumulates `p * log(p) * M_LOG2E` and
            // negates once at return. Preserve this order and its fused update.
            entropy = (probability * probability.ln()).mul_add(std::f64::consts::LOG2_E, entropy);
        }
    }
    -entropy
}
