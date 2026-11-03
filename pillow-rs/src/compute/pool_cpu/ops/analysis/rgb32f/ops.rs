use crate::compute::Architecture;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{Mode, Rgb32FImage};

use super::super::helpers::get_data::{
    self as helpers, GetDataOutput, GetDataRequest,
};
use super::default::get_data_impl;

pub(crate) fn execute_get_data(
    image: &Rgb32FImage,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageRgb32F, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(Mode::ImageRgb32F, architecture)),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    rgb32f,
    ImageRgb32F,
    crate::raster::Rgb32FImage,
);

// NO_PILLOW_SUPPORT: native-width multiband extrema are a core extension.
crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageRgb32F,
    crate::raster::Rgb32FImage,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::f32_bands,
);

// NO_PILLOW_SUPPORT: native float multiband histograms extend the core API.
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageRgb32F,
    crate::raster::Rgb32FImage,
    crate::compute::pool_cpu::ops::analysis::rgb32f::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageRgb32F,
    crate::raster::Rgb32FImage,
    crate::compute::pool_cpu::ops::analysis::rgb32f::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageRgb32F,
    crate::raster::Rgb32FImage,
    crate::compute::pool_cpu::ops::analysis::rgb32f::default::stat_impl::execute,
);
// NO_PILLOW_SUPPORT: the Rust/JS byte-valued extrema API exposes a compatibility projection.
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::
    declare_getextrema_bytes_owner!(
    ImageRgb32F,
    crate::raster::Rgb32FImage,
    crate::compute::pool_cpu::ops::analysis::rgb32f::default::extrema_bytes_impl::execute,
);
