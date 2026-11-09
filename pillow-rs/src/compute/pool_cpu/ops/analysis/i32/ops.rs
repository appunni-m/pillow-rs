use crate::compute::Architecture;
use crate::compute::registry::support::StrategySelection;
use crate::error::PilError;
use crate::raster::{Mode, RgbaImage};

use super::super::helpers::get_data::{self as helpers, GetDataOutput, GetDataRequest};
use super::default::get_data_impl;

pub(crate) fn execute_get_data(
    image: &RgbaImage,
    selection: StrategySelection,
    request: GetDataRequest<'_>,
) -> Result<GetDataOutput, PilError> {
    let architecture = helpers::selected_architecture(selection, Mode::ImageI32, &request)?;
    match architecture {
        Architecture::Default => match request {
            GetDataRequest::Full => Ok(GetDataOutput::Bytes(get_data_impl::get_data(image)?)),
            GetDataRequest::Band { index, output } => {
                get_data_impl::write_band(image, index, output)?;
                Ok(GetDataOutput::BandWritten)
            }
        },
        architecture => Err(helpers::missing_architecture(Mode::ImageI32, architecture)),
    }
}

pub(crate) fn scalar_samples(
    image: &RgbaImage,
    selection: StrategySelection,
) -> Result<crate::image::ScalarImageSamples, PilError> {
    let request = GetDataRequest::Full;
    let architecture = helpers::selected_architecture(selection, Mode::ImageI32, &request)?;
    match architecture {
        Architecture::Default => get_data_impl::scalar_samples(image),
        architecture => Err(helpers::missing_architecture(Mode::ImageI32, architecture)),
    }
}
crate::compute::pool_cpu::ops::analysis::helpers::bbox::declare_getbbox_owner!(
    i32,
    ImageI32,
    crate::raster::RgbaImage,
    Pillow,
);

crate::compute::pool_cpu::ops::analysis::helpers::extrema::declare_getextrema_owner!(
    ImageI32,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::helpers::extrema::i32_scalar,
);

crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_owner!(
    ImageI32,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::i32::default::histogram_impl::execute,
);

pub(crate) fn execute_histogram_with_extrema(
    image: &RgbaImage,
    selection: StrategySelection,
    mask_raw: Option<&[u8]>,
    minimum: i32,
    maximum: i32,
) -> Result<Vec<u32>, PilError> {
    match crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::
        selected_histogram_architecture(selection, Mode::ImageI32)?
    {
        Architecture::Default => super::default::histogram_impl::execute_with_extrema(
            image, mask_raw, minimum, maximum,
        ),
        architecture => Err(PilError::InternalError(format!(
            "Histogram has no {:?} {architecture:?} strategy",
            Mode::ImageI32,
        ))),
    }
}

crate::compute::pool_cpu::ops::analysis::helpers::getcolors_execution::declare_getcolors_owner!(
    ImageI32,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::i32::default::getcolors_impl::execute,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_owner!(
    ImageI32,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::i32::default::stat_impl::execute,
);
// NO_PILLOW_SUPPORT: the Rust/JS byte-valued extrema API exposes a compatibility projection.
crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_owner!(
    ImageI32,
    crate::raster::RgbaImage,
    crate::compute::pool_cpu::ops::analysis::i32::default::extrema_bytes_impl::execute,
);

pub(crate) const GET_PIXEL_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::pillow_get_pixel_support(
        crate::raster::Mode::ImageI32,
        crate::compute::registry::support::PixelValueShape::I32Scalar,
    );

pub(crate) const GET_PIXEL_RGBA_SUPPORT: crate::compute::registry::support::ModeSupport =
    crate::compute::pool_cpu::ops::analysis::helpers::pixel_access::rgba_get_pixel_support(
        crate::raster::Mode::ImageI32,
    );

pub(crate) fn execute_get_pixel(
    image: &crate::raster::RgbaImage,
    x: u32,
    y: u32,
    selection: crate::compute::registry::support::StrategySelection,
) -> Result<crate::image::FormattedPixelValue, crate::error::PilError> {
    super::super::helpers::pixel_access::validate_default_selection(
        selection,
        crate::compute::registry::support::OperationId::GetPixel,
        crate::raster::Mode::ImageI32,
    )?;
    super::default::get_pixel_impl::execute(image, x, y)
}
crate::compute::pool_cpu::ops::analysis::helpers::getcolors_support::declare_getcolors_support!(
    ImageI32, pillow,
);
// NO_PILLOW_SUPPORT: Rust/JS exposes byte-valued extrema for this typed mode.

crate::compute::pool_cpu::ops::analysis::helpers::extrema_bytes_execution::declare_getextrema_bytes_support!(
    ImageI32, core_only,
);
crate::compute::pool_cpu::ops::analysis::helpers::extrema_execution::declare_getextrema_support!(
    ImageI32, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::histogram_execution::declare_histogram_support!(
    ImageI32, 1, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::stat_execution::declare_stat_support!(
    ImageI32, pillow,
);
crate::compute::pool_cpu::ops::analysis::helpers::split_support::declare_split_support!(
    ImageI32, pillow, Identity,
);
crate::compute::pool_cpu::ops::analysis::helpers::get_channel_support::declare_get_channel_route!(
    ImageI32, 1, Rejected, Child, Luma8,
);

crate::compute::pool_cpu::ops::analysis::helpers::get_data_support::declare_get_data_support!(
    ImageI32, 1, Pillow, Pillow, Rejected,
);

crate::compute::pool_cpu::ops::analysis::helpers::projection_support::declare_projection_support!(
    ImageI32, Pillow,
);
