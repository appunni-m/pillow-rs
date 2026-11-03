use crate::compute::Architecture;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{Mode, GrayImage};

use super::super::helpers::get_data::{
    self as helpers, GetDataOutput, GetDataRequest,
};
use super::default::get_data_impl;

pub(crate) fn execute_get_data(
    image: &GrayImage,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageLuma8, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(Mode::ImageLuma8, architecture)),
    }
}

pub(crate) fn get_data_view(
    image: &GrayImage,
    selection: StrategySelection,
) -> Result<&[u8], PilError> {
    let request = GetDataRequest::Full;
    let architecture = helpers::selected_architecture(selection, Mode::ImageLuma8, &request)?;
    match architecture {
        Architecture::Default => Ok(get_data_impl::get_data_view(image)),
        architecture => Err(helpers::missing_architecture(Mode::ImageLuma8, architecture)),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    luma8,
    ImageLuma8,
    crate::raster::GrayImage,
);

crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageLuma8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::bytes,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageLuma8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::luma8::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageLuma8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::luma8::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageLuma8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::luma8::default::stat_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::
    declare_getextrema_bytes_owner!(
    ImageLuma8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::luma8::default::extrema_bytes_impl::execute,
);
