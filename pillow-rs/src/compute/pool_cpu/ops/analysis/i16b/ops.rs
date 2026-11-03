use crate::compute::Architecture;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::Mode;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image;

use super::super::helpers::get_data::{
    self as helpers, GetDataOutput, GetDataRequest,
};
use super::default::get_data_impl;

pub(crate) fn execute_get_data(
    image: &Luma16Image,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageI16B, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(Mode::ImageI16B, architecture)),
    }
}

pub(crate) fn integer_samples(
    image: &Luma16Image,
    selection: StrategySelection,
) -> Result<Vec<i32>, PilError> {
    let request = GetDataRequest::Full;
    let architecture = helpers::selected_architecture(selection, Mode::ImageI16B, &request)?;
    match architecture {
        Architecture::Default => get_data_impl::integer_samples(image),
        architecture => Err(helpers::missing_architecture(Mode::ImageI16B, architecture)),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    i16b,
    ImageI16B,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageI16B,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
    crate::compute::pool_cpu::ops::analysis::i16b::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageI16B,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
    crate::compute::pool_cpu::ops::analysis::i16b::default::stat_impl::execute,
);
// NO_PILLOW_SUPPORT: the Rust/JS byte-valued extrema API exposes a compatibility projection.
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::
    declare_getextrema_bytes_owner!(
    ImageI16B,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
    crate::compute::pool_cpu::ops::analysis::i16b::default::extrema_bytes_impl::execute,
);
