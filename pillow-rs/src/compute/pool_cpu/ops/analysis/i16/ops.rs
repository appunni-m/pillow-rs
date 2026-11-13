use crate::compute::Architecture;
use crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::Mode;

use super::super::helpers::get_data::{self as helpers, GetDataOutput, GetDataRequest};
use super::default::get_data_impl;

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_data(
    image: &Luma16Image,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageI16, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(Mode::ImageI16, architecture)),
    }
}

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn integer_samples(
    image: &Luma16Image,
    selection: StrategySelection,
) -> Result<Vec<i32>, PilError> {
    let request = GetDataRequest::Full;
    let architecture = helpers::selected_architecture(selection, Mode::ImageI16, &request)?;
    match architecture {
        Architecture::Default => get_data_impl::integer_samples(image),
        architecture => Err(helpers::missing_architecture(Mode::ImageI16, architecture)),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    i16,
    ImageI16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
    Pillow,
);

crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageI16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::u16_scalar,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageI16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
    crate::compute::pool_cpu::ops::analysis::i16::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageI16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
    crate::compute::pool_cpu::ops::analysis::i16::default::stat_impl::execute,
);
// NO_PILLOW_SUPPORT: the Rust/JS byte-valued extrema API exposes a compatibility projection.
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_owner!(
    ImageI16,
    crate::compute::pool_cpu::ops::analysis::helpers::get_data::Luma16Image,
    crate::compute::pool_cpu::ops::analysis::i16::default::extrema_bytes_impl::execute,
);

pub(crate) const GET_PIXEL_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::pillow_get_pixel_support(
        crate::raster::Mode::ImageI16,
        crate::compute::registry::support::PixelValueShape::I32Scalar,
    );

pub(crate) const GET_PIXEL_RGBA_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::rgba_get_pixel_support(
        crate::raster::Mode::ImageI16,
    );

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
pub(crate) fn execute_get_pixel(
    image: &crate::raster::ImageBuffer<crate::raster::Luma<u16>, Vec<u16>>,
    x: u32,
    y: u32,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<crate::image::FormattedPixelValue, crate::error::PilError> {
    super::super::helpers::pixel_access::validate_default_selection(
        selection,
        crate::compute::registry::support::OperationId::GetPixel,
        crate::raster::Mode::ImageI16,
    )?;
    super::default::get_pixel_impl::execute(image, x, y)
}
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::declare_getcolors_support!(
    ImageI16, rejected,
);
// NO_PILLOW_SUPPORT: Rust/JS exposes byte-valued extrema for this typed mode.

crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_support!(
    ImageI16, core_only,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::declare_getextrema_support!(
    ImageI16, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_support!(
    ImageI16, 1, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_support!(
    ImageI16, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::split_support::declare_split_support!(
    ImageI16, pillow, Identity,
);
// NO_PILLOW_SUPPORT: Pillow rejects typed I;16 GetChannel; Rust preserves native identity.
crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::declare_get_channel_route!(
    ImageI16,
    1,
    I16CoreOnly,
    Identity,
    Preserve,
);

crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::declare_get_data_support!(
    ImageI16, 1, Pillow, Pillow, Rejected,
);

// VERIFY: source=reviewed@2026-11-13; parity=pending; benchmark=pending
crate::compute::pool_cpu::ops::analysis::helpers::projection_support::declare_projection_support!(
    ImageI16, Pillow,
);
