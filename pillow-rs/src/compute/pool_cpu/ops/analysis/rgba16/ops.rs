use crate::compute::Architecture;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::Mode;

use super::super::helpers::get_data::{self as helpers, GetDataOutput, GetDataRequest};
use super::default::get_data_impl;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_data(
    image: &Rgba16Image,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageRgba16, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(
            Mode::ImageRgba16,
            architecture,
        )),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    rgba16,
    ImageRgba16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image,
    CoreOnly,
);

// NO_PILLOW_SUPPORT: native-width multiband extrema are a core extension.
crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageRgba16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::u16_bands,
);

// NO_PILLOW_SUPPORT: native-width multiband histograms extend the core API.
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageRgba16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image,
    crate::compute::pool_cpu::ops::analysis::rgba16::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageRgba16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image,
    crate::compute::pool_cpu::ops::analysis::rgba16::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageRgba16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image,
    crate::compute::pool_cpu::ops::analysis::rgba16::default::stat_impl::execute,
);
// NO_PILLOW_SUPPORT: the Rust/JS byte-valued extrema API exposes a compatibility projection.
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_owner!(
    ImageRgba16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Rgba16Image,
    crate::compute::pool_cpu::ops::analysis::rgba16::default::extrema_bytes_impl::execute,
);

pub(crate) const GET_PIXEL_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::core_get_pixel_support(
        crate::raster::Mode::ImageRgba16,
        crate::compute::registry::support::PixelValueShape::U16Components { bands: 4 },
    );

pub(crate) const GET_PIXEL_RGBA_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::rgba_get_pixel_support(
        crate::raster::Mode::ImageRgba16,
    );

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_pixel(
    image: &crate::raster::ImageBuffer<crate::raster::Rgba<u16>, Vec<u16>>,
    x: u32,
    y: u32,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<crate::image::FormattedPixelValue, crate::error::PilError> {
    super::super::helpers::pixel_access::validate_default_selection(
        selection,
        crate::compute::registry::support::OperationId::GetPixel,
        crate::raster::Mode::ImageRgba16,
    )?;
    super::default::get_pixel_impl::execute(image, x, y)
}
// NO_PILLOW_SUPPORT: native high-depth GetColors returns native-width tuples.

crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::declare_getcolors_support!(
    ImageRgba16,
    core_only,
);
// NO_PILLOW_SUPPORT: Rust/JS exposes byte-valued extrema for this typed mode.

crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_support!(
    ImageRgba16,
    core_only,
);
// NO_PILLOW_SUPPORT: native-width multiband GetExtrema extends Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::declare_getextrema_support!(
    ImageRgba16,
    core_only,
);
// NO_PILLOW_SUPPORT: native high-depth Histogram and Entropy extend Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_support!(
    ImageRgba16,
    4,
    core_only,
);
// NO_PILLOW_SUPPORT: native-sample Stat extends Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_support!(
    ImageRgba16,
    core_only,
);
// NO_PILLOW_SUPPORT: exact-mode native high-depth Split extends Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::split_support::declare_split_support!(
    ImageRgba16,
    core_only,
    Bands,
);
// NO_PILLOW_SUPPORT: exact-mode native high-depth GetChannel extends Pillow.

crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::declare_get_channel_route!(
    ImageRgba16,
    4,
    U16CoreOnly,
    Child,
    I16,
);

// NO_PILLOW_SUPPORT: native high-depth GetData paths preserve their typed carriers.
crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::declare_get_data_support!(
    ImageRgba16,
    4,
    CoreOnly,
    CoreOnly,
    CoreOnly,
);

// NO_PILLOW_SUPPORT: native high-depth projection extends the Pillow mode contract.
// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
crate::compute::pool_cpu::ops::analysis::helpers::projection_support::declare_projection_support!(
    ImageRgba16,
    CoreOnly,
);
