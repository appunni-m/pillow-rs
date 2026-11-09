use crate::compute::Architecture;
use crate::compute::registry::support::{OperationId, StrategySelection};
use crate::error::PilError;
use crate::raster::{GrayImage, Mode};

use super::super::helpers::get_data::{self as helpers, GetDataOutput, GetDataRequest};
use super::default::get_data_impl;

pub(crate) fn execute_get_data(
    image: &GrayImage,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImagePalette8, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(
            Mode::ImagePalette8,
            architecture,
        )),
    }
}

pub(crate) fn get_data_view(
    image: &GrayImage,
    selection: StrategySelection,
) -> Result<&[u8], PilError> {
    let request = GetDataRequest::Full;
    let architecture = helpers::selected_architecture(selection, Mode::ImagePalette8, &request)?;
    match architecture {
        Architecture::Default => Ok(get_data_impl::get_data_view(image)),
        architecture => Err(helpers::missing_architecture(
            Mode::ImagePalette8,
            architecture,
        )),
    }
}

pub(crate) fn write_palette_indices_band(
    indices: &[u8],
    selection: StrategySelection,
    band: usize,
    output: &mut [u8],
) -> Result<(), PilError> {
    let architecture = helpers::selected_operation_architecture(
        selection,
        Mode::ImagePalette8,
        OperationId::GetDataBandInto,
    )?;
    match architecture {
        Architecture::Default => get_data_impl::write_palette_indices_band(indices, band, output),
        architecture => Err(helpers::missing_architecture(
            Mode::ImagePalette8,
            architecture,
        )),
    }
}

pub(crate) fn palette_indices_view<'a>(
    indices: &'a [u8],
    selection: StrategySelection,
) -> Result<&'a [u8], PilError> {
    let architecture = helpers::selected_operation_architecture(
        selection,
        Mode::ImagePalette8,
        OperationId::GetData,
    )?;
    match architecture {
        Architecture::Default => Ok(get_data_impl::palette_indices_view(indices)),
        architecture => Err(helpers::missing_architecture(
            Mode::ImagePalette8,
            architecture,
        )),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    palette8,
    ImagePalette8,
    crate::raster::GrayImage,
    Pillow,
);

crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImagePalette8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::bytes,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImagePalette8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::palette8::default::histogram_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImagePalette8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::palette8::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImagePalette8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::palette8::default::stat_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_owner!(
    ImagePalette8,
    crate::raster::GrayImage,
    crate::compute::pool_cpu::ops::analysis::palette8::default::extrema_bytes_impl::execute,
);

pub(crate) const GET_PIXEL_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::pillow_get_pixel_support(
        crate::raster::Mode::ImagePalette8,
        crate::compute::registry::support::PixelValueShape::U8Scalar,
    );

pub(crate) const GET_PIXEL_RGBA_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::rgba_get_pixel_support(
        crate::raster::Mode::ImagePalette8,
    );

pub(crate) fn execute_get_pixel(
    image: &crate::raster::GrayImage,
    x: u32,
    y: u32,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<crate::image::FormattedPixelValue, crate::error::PilError> {
    super::super::helpers::pixel_access::validate_default_selection(
        selection,
        crate::compute::registry::support::OperationId::GetPixel,
        crate::raster::Mode::ImagePalette8,
    )?;
    super::default::get_pixel_impl::execute(image, x, y)
}
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::declare_getcolors_support!(
    ImagePalette8,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_support!(
    ImagePalette8,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::declare_getextrema_support!(
    ImagePalette8,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_support!(
    ImagePalette8,
    1,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_support!(
    ImagePalette8,
    pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::split_support::declare_split_support!(
    ImagePalette8,
    pillow,
    Palette,
);
crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::declare_get_channel_route!(
    ImagePalette8,
    1,
    Pillow,
    Palette,
    Preserve,
);

crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::declare_get_data_support!(
    ImagePalette8,
    1,
    Pillow,
    Pillow,
    Pillow,
);

crate::compute::pool_cpu::ops::analysis::helpers::projection_support::declare_projection_support!(
    ImagePalette8,
    Pillow,
);
