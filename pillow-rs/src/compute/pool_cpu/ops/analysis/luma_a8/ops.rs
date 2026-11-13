use crate::compute::Architecture;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{GrayAlphaImage, Mode};

use super::super::helpers::get_data::{self as helpers, GetDataOutput, GetDataRequest};
use super::default::get_data_impl;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_data(
    image: &GrayAlphaImage,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageLumaA8, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(
            Mode::ImageLumaA8,
            architecture,
        )),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    luma_a8,
    ImageLumaA8,
    crate::raster::GrayAlphaImage,
    Pillow,
);

crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageLumaA8,
    crate::raster::GrayAlphaImage,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::bytes,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageLumaA8,
    crate::raster::GrayAlphaImage,
    crate::compute::pool_cpu::ops::analysis::luma_a8::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageLumaA8,
    crate::raster::GrayAlphaImage,
    crate::compute::pool_cpu::ops::analysis::luma_a8::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageLumaA8,
    crate::raster::GrayAlphaImage,
    crate::compute::pool_cpu::ops::analysis::luma_a8::default::stat_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_owner!(
    ImageLumaA8,
    crate::raster::GrayAlphaImage,
    crate::compute::pool_cpu::ops::analysis::luma_a8::default::extrema_bytes_impl::execute,
);

pub(crate) const GET_PIXEL_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::pillow_get_pixel_support(
        crate::raster::Mode::ImageLumaA8,
        crate::compute::registry::support::PixelValueShape::U8Components { bands: 2 },
    );

pub(crate) const GET_PIXEL_RGBA_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::rgba_get_pixel_support(
        crate::raster::Mode::ImageLumaA8,
    );

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_pixel(
    image: &crate::raster::GrayAlphaImage,
    x: u32,
    y: u32,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<crate::image::FormattedPixelValue, crate::error::PilError> {
    super::super::helpers::pixel_access::validate_default_selection(
        selection,
        crate::compute::registry::support::OperationId::GetPixel,
        crate::raster::Mode::ImageLumaA8,
    )?;
    super::default::get_pixel_impl::execute(image, x, y)
}
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::declare_getcolors_support!(
    ImageLumaA8,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_support!(
    ImageLumaA8,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::declare_getextrema_support!(
    ImageLumaA8,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_support!(
    ImageLumaA8,
    2,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_support!(
    ImageLumaA8,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::split_support::declare_split_support!(
    ImageLumaA8,
    pillow,
    Bands,
);
crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::declare_get_channel_route!(
    ImageLumaA8,
    2,
    Pillow,
    Child,
    Luma8,
);

crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::declare_get_data_support!(
    ImageLumaA8,
    2,
    Pillow,
    Pillow,
    Pillow,
);

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
crate::compute::pool_cpu::ops::analysis::helpers::projection_support::declare_projection_support!(
    ImageLumaA8,
    Pillow,
);
