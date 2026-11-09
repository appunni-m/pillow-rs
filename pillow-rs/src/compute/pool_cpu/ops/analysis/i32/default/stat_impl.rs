use crate::compute::pool_cpu::ops::analysis::helpers::stat::{self, StatOutput, StatRequest};
use crate::error::PilError;
use crate::raster::RgbaImage;

pub(crate) fn execute(image: &RgbaImage, request: StatRequest<'_>) -> Result<StatOutput, PilError> {
    match request {
        StatRequest::Scalar => Ok(vec![stat::scalar_i32(image)?]),
        StatRequest::FormattedHistogram(histogram) => Ok(stat::formatted_histogram(histogram)),
        StatRequest::Histogram(_) => Err(PilError::InternalError(
            "Stat scalar implementation received histogram input".into(),
        )),
    }
}
