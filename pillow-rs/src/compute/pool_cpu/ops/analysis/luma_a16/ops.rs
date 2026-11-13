use crate::compute::Architecture;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::Mode;

use super::super::helpers::get_data::{self as helpers, GetDataOutput, GetDataRequest};
use super::default::get_data_impl;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_data(
    image: &LumaA16Image,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageLumaA16, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(
            Mode::ImageLumaA16,
            architecture,
        )),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    luma_a16,
    ImageLumaA16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image,
    CoreOnly,
);

// NO_PILLOW_SUPPORT: native-width multiband extrema are a core extension.
crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageLumaA16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::u16_bands,
);

// NO_PILLOW_SUPPORT: native-width multiband histograms extend the core API.
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageLumaA16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image,
    crate::compute::pool_cpu::ops::analysis::luma_a16::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageLumaA16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image,
    crate::compute::pool_cpu::ops::analysis::luma_a16::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageLumaA16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image,
    crate::compute::pool_cpu::ops::analysis::luma_a16::default::stat_impl::execute,
);
// NO_PILLOW_SUPPORT: the Rust/JS byte-valued extrema API exposes a compatibility projection.
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_owner!(
    ImageLumaA16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::LumaA16Image,
    crate::compute::pool_cpu::ops::analysis::luma_a16::default::extrema_bytes_impl::execute,
);

pub(crate) const GET_PIXEL_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::core_get_pixel_support(
        crate::raster::Mode::ImageLumaA16,
        crate::compute::registry::support::PixelValueShape::U16Components { bands: 2 },
    );

pub(crate) const GET_PIXEL_RGBA_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::rgba_get_pixel_support(
        crate::raster::Mode::ImageLumaA16,
    );

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_pixel(
    image: &crate::raster::ImageBuffer<crate::raster::LumaA<u16>, Vec<u16>>,
    x: u32,
    y: u32,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<crate::image::FormattedPixelValue, crate::error::PilError> {
    super::super::helpers::pixel_access::validate_default_selection(
        selection,
        crate::compute::registry::support::OperationId::GetPixel,
        crate::raster::Mode::ImageLumaA16,
    )?;
    super::default::get_pixel_impl::execute(image, x, y)
}
// NO_PILLOW_SUPPORT: native high-depth GetColors returns native-width tuples.

crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::declare_getcolors_support!(
    ImageLumaA16,
    core_only,
);
// NO_PILLOW_SUPPORT: Rust/JS exposes byte-valued extrema for this typed mode.

crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_support!(
    ImageLumaA16,
    core_only,
);
// NO_PILLOW_SUPPORT: native-width multiband GetExtrema extends Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::declare_getextrema_support!(
    ImageLumaA16,
    core_only,
);
// NO_PILLOW_SUPPORT: native high-depth Histogram and Entropy extend Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_support!(
    ImageLumaA16,
    2,
    core_only,
);
// NO_PILLOW_SUPPORT: native-sample Stat extends Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_support!(
    ImageLumaA16,
    core_only,
);
// NO_PILLOW_SUPPORT: exact-mode native high-depth Split extends Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::split_support::declare_split_support!(
    ImageLumaA16,
    core_only,
    Bands,
);
// NO_PILLOW_SUPPORT: exact-mode native high-depth GetChannel extends Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::declare_get_channel_route!(
    ImageLumaA16,
    2,
    U16CoreOnly,
    Child,
    I16,
);

// NO_PILLOW_SUPPORT: native high-depth GetData paths preserve their typed carriers.
crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::declare_get_data_support!(
    ImageLumaA16,
    2,
    CoreOnly,
    CoreOnly,
    CoreOnly,
);

// NO_PILLOW_SUPPORT: native high-depth projection extends the Pillow mode contract.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
crate::compute::pool_cpu::ops::analysis::helpers::projection_support::declare_projection_support!(
    ImageLumaA16,
    CoreOnly,
);
