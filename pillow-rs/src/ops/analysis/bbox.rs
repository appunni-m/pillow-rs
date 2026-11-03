//! Public GetBBox orchestration over the prepared CPU analysis strategy.

use crate::compute::registry::support::StrategySelection;
use crate::compute::{Backend, PipelineOperationTelemetryCapture};
use crate::error::PilError;
use crate::image::Image;

impl Image {
    /// Returns the bounding box of non-zero image content.
    ///
    /// The result is a four-element tuple with right and bottom exclusive,
    /// matching Pillow getbbox. None means every inspected pixel is zero.
    /// When alpha_only is true and the exact mode has alpha, only that band
    /// is inspected.
    ///
    /// # Errors
    ///
    /// Returns an error when materialization or strategy preparation fails.
    pub fn getbbox(&self, alpha_only: bool) -> Result<Option<(u32, u32, u32, u32)>, PilError> {
        let (requested_backend, context) = self.cpu_arch_execution_route_advisory();
        let mut operation_receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared()?;
        operation_receipts.capture();
        let telemetry = if Backend::pipeline_telemetry_enabled() {
            // TODO(pillow-rs#3ptb): define aggregate timing/resource semantics
            // for lazy source graphs; this summary covers the terminal scan.
            let telemetry = crate::compute::begin_eager_cpu_operation_telemetry(
                "GetBBox",
                requested_backend,
            );
            operation_receipts.restore();
            Some(telemetry)
        } else {
            None
        };
        let result: Result<
            (Option<(u32, u32, u32, u32)>, StrategySelection),
            PilError,
        > = (|| {
            let prepared = crate::compute::pool_cpu::ops::analysis::ops::prepare_get_bbox(
                image.as_ref(),
                alpha_only,
                &context,
            )?;
            let selection = *prepared.selection();
            Ok((prepared.execute()?, selection))
        })();
        if let Some(telemetry) = telemetry {
            match result {
                Ok((bounds, selection)) => {
                    crate::compute::finish_eager_cpu_operation_telemetry_with_selection(
                        telemetry,
                        0,
                        "GetBBox",
                        selection,
                    );
                    Ok(bounds)
                }
                Err(error) => {
                    crate::compute::cancel_eager_cpu_operation_telemetry(telemetry);
                    Err(error)
                }
            }
        } else {
            result.map(|(bounds, _)| bounds)
        }
    }
}
