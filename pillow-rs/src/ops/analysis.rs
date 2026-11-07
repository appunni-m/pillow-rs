//! Pillow-compatible image analysis operations.
//!
//! These methods materialize lazy pipelines before scanning pixels. Results are
//! returned as Rust primitives instead of Python tuples or lists.
use crate::compute::registry::support::OperationId;
use crate::compute::PipelineOperationTelemetryCapture;
use crate::error::PilError;
use crate::image::Image;
use crate::raster::Mode;

/// Host-neutral optional mask input for image analysis operations.
#[derive(Debug, Clone)]
pub enum ImageAnalysisMask {
    /// No mask was supplied.
    None,
    /// A mask image extracted by a binding.
    Image(Box<Image>),
    /// A truthy non-image value was supplied, retaining its host type name so
    /// bindings can expose Pillow's attribute error without inspecting host
    /// objects in the core.
    Invalid(String),
}

/// Native I/F sample range for Histogram and its Entropy composition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HistogramExtrema {
    /// Signed 32-bit lower and upper sample values for `I` mode.
    ImageI32 {
        /// Minimum signed sample value in the image.
        minimum: i32,
        /// Maximum signed sample value in the image.
        maximum: i32,
    },
    /// Float32 lower and upper sample values for `F` mode.
    ImageF32 {
        /// Minimum float sample value in the image.
        minimum: f32,
        /// Maximum float sample value in the image.
        maximum: f32,
    },
}

mod bbox;
mod histogram;

/// Validate a Pillow transparency/statistics mask without touching host types.
pub fn validate_transparency_mask(image: &Image, mask: &Image) -> Result<(), PilError> {
    let mode = mask.mode_identity()?;
    let size = mask.size()?;
    if !matches!(mode, Mode::One | Mode::ImageLuma8) {
        return Err(PilError::ValueError("bad transparency mask".into()));
    }
    // Pillow's ImageOps mask path distinguishes an unsupported mode from a
    // same-mode size mismatch: the former is "bad transparency mask", while
    // the latter is "images do not match".
    if size != image.size()? {
        return Err(PilError::ValueError("images do not match".into()));
    }
    Ok(())
}

impl Image {
    /// Computes a histogram after validating a host-neutral optional mask.
    pub fn histogram_with_input(&self, mask: ImageAnalysisMask) -> Result<Vec<u32>, PilError> {
        self.histogram_with_input_and_extrema(mask, None)
    }

    /// Computes a histogram with Pillow's optional native `I`/`F` range.
    pub fn histogram_with_input_and_extrema(
        &self,
        mask: ImageAnalysisMask,
        extrema: Option<HistogramExtrema>,
    ) -> Result<Vec<u32>, PilError> {
        match mask {
            ImageAnalysisMask::None => self.histogram_with_extrema(extrema),
            // Pillow supplies a fixed (0, 0) range to the masked ImagingCore
            // route; the mask itself is unsupported for I/F modes.
            ImageAnalysisMask::Image(mask) => {
                self.histogram_with_mask_and_extrema(Some(&mask), None)
            }
            ImageAnalysisMask::Invalid(type_name) => Err(PilError::AttributeError(format!(
                "'{type_name}' object has no attribute 'load'"
            ))),
        }
    }

    /// Computes entropy after validating a host-neutral optional mask.
    pub fn entropy_with_input(&self, mask: ImageAnalysisMask) -> Result<f64, PilError> {
        self.entropy_with_input_and_extrema(mask, None)
    }

    /// Computes entropy using an optional native `I`/`F` histogram range.
    // NO_PILLOW_SUPPORT: Pillow's public `Image.entropy` accepts a mask only.
    pub fn entropy_with_input_and_extrema(
        &self,
        mask: ImageAnalysisMask,
        extrema: Option<HistogramExtrema>,
    ) -> Result<f64, PilError> {
        match mask {
            ImageAnalysisMask::None => self.entropy_with_extrema(extrema),
            ImageAnalysisMask::Image(mask) => {
                self.entropy_with_mask_and_extrema(Some(&mask), None)
            }
            ImageAnalysisMask::Invalid(type_name) => Err(PilError::AttributeError(format!(
                "'{type_name}' object has no attribute 'load'"
            ))),
        }
    }
}

impl Image {
    /// Returns minimum and maximum byte values per result lane.
    ///
    /// Lanes follow the existing Rust/JS result contract. Wide modes project
    /// native samples to bytes; Python's Pillow-facing method uses typed
    /// results from `getextrema_formatted` instead.
    ///
    /// # Errors
    ///
    /// Returns [`PilError`] when materialization fails.
    pub fn getextrema(&self) -> Result<Vec<(u8, u8)>, PilError> {
        let mode = self.mode_identity()?;
        OperationId::GetExtremaBytes
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
                "GetExtremaBytes",
                requested_backend,
            );
            operation_receipts.restore();
            Some(telemetry)
        } else {
            None
        };
        let result: Result<
            (
                Vec<(u8, u8)>,
                crate::compute::registry::support::StrategySelection,
            ),
            PilError,
        > = (|| {
            let prepared = crate::compute::pool_cpu::ops::analysis::ops::prepare_get_extrema_bytes(
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
                        extrema
                            .len()
                            .saturating_mul(std::mem::size_of::<(u8, u8)>()),
                        "GetExtremaBytes",
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
