use crate::compute::Architecture;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::Mode;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image;

use super::super::helpers::get_data::{
    self as helpers, GetDataOutput, GetDataRequest,
};
use super::default::get_data_impl;

pub(crate) fn execute_get_data(
    image: &Rgb16Image,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageRgb16, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(Mode::ImageRgb16, architecture)),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    rgb16,
    ImageRgb16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image,
);

// NO_PILLOW_SUPPORT: native-width multiband extrema are a core extension.
crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageRgb16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::u16_bands,
);

// NO_PILLOW_SUPPORT: native-width multiband histograms extend the core API.
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageRgb16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image,
    crate::compute::pool_cpu::ops::analysis::rgb16::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageRgb16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image,
    crate::compute::pool_cpu::ops::analysis::rgb16::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageRgb16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image,
    crate::compute::pool_cpu::ops::analysis::rgb16::default::stat_impl::execute,
);
// NO_PILLOW_SUPPORT: the Rust/JS byte-valued extrema API exposes a compatibility projection.
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::
    declare_getextrema_bytes_owner!(
    ImageRgb16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgb16Image,
    crate::compute::pool_cpu::ops::analysis::rgb16::default::extrema_bytes_impl::execute,
);

pub(crate) const GET_PIXEL_SUPPORT:
    crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::core_get_pixel_support(
        crate::raster::Mode::ImageRgb16,
        crate::compute::registry::support::PixelValueShape::U16Components { bands: 3 },
    );

pub(crate) const GET_PIXEL_RGBA_SUPPORT:
    crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::rgba_get_pixel_support(
        crate::raster::Mode::ImageRgb16,
    );

pub(crate) fn execute_get_pixel(
    image: &crate::raster::ImageBuffer<crate::raster::Rgb<u16>, Vec<u16>>,
    x: u32,
    y: u32,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<crate::image::FormattedPixelValue, crate::error::PilError> {
    super::super::helpers::pixel_access::validate_default_selection(
        selection,
        crate::compute::registry::support::OperationId::GetPixel,
        crate::raster::Mode::ImageRgb16,
    )?;
    super::default::get_pixel_impl::execute(image, x, y)
}
