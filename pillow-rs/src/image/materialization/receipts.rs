use std::sync::Arc;

use crate::compute::PipelineOperationTelemetryCapture;
use crate::error::PilError;
use crate::raster::DynamicImage;

use super::Image;

impl Image {
    /// Materializes operation-ready pixels while capturing their source graph
    /// receipts for an eager child operation.
    pub(crate) fn materialized_shared_for_ops_with_receipts(
        &self,
    ) -> Result<(Arc<DynamicImage>, PipelineOperationTelemetryCapture), PilError> {
        let mut receipts = PipelineOperationTelemetryCapture::begin();
        let image = self.materialized_shared_for_ops()?;
        receipts.capture();
        Ok((image, receipts))
    }
}
