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
    let architecture = helpers::selected_architecture(selection, Mode::One, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(Mode::One, architecture)),
    }
}

pub(crate) fn get_data_view(
    image: &GrayImage,
    selection: StrategySelection,
) -> Result<&[u8], PilError> {
    let request = GetDataRequest::Full;
    let architecture = helpers::selected_architecture(selection, Mode::One, &request)?;
    match architecture {
        Architecture::Default => Ok(get_data_impl::get_data_view(image)),
        architecture => Err(helpers::missing_architecture(Mode::One, architecture)),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    one,
    One,
    crate::raster::GrayImage,
);

crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    One,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::bytes,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    One,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::one::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    One,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::one::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    One,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::one::default::stat_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::
    declare_getextrema_bytes_owner!(
    One,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::one::default::extrema_bytes_impl::execute,
);
