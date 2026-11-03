pub(crate) type Input = crate::raster::GrayImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy::declare_invert_native_mode;
use crate::compute::Architecture;
use crate::compute::registry::support::PaletteInput;
use crate::error::PilError;
use crate::raster::GrayImage;
use crate::compute::pool_cpu::ops::imageops::helpers::strategy;
use crate::compute::registry::support::{
    ImplementationSupport, InputContract, ModeSupport, OperationId, OutputModeRule,
};
use crate::raster::Mode;

// NO_PILLOW_SUPPORT: Pillow's ImageOps.invert rejects P. This Rust strategy
// inverts the native palette indices and preserves the attached palette.
declare_invert_native_mode!(
    ImagePalette8,
    Input,
    1,
    1,
    Some("ImageOps.invert extends Pillow with native P index inversion"),
);

pub(crate) fn execute_quantize(
    image: &GrayImage,
    palette: PaletteInput<'_>,
    width: u32,
    height: u32,
    colors: usize,
    kmeans: u32,
    method: u32,
    architecture: Architecture,
) -> Result<(Vec<u8>, Vec<u8>, Option<Vec<u8>>), PilError> {
    match architecture {
        Architecture::Default => super::default::quantize_impl::execute(
            image, palette, width, height, colors, kmeans, method,
        ),
        _ => Err(PilError::InternalError(
            "P Quantize selected an unimplemented architecture".into(),
        )),
    }
}

const POSTERIZE_INPUTS: &[InputContract] = &[
    InputContract::PosterizeBits,
    InputContract::CheckedByteDimensions {
        bytes_per_pixel: 1,
        allow_empty: true,
    },
];
const SOLARIZE_INPUTS: &[InputContract] = &[InputContract::CheckedByteDimensions {
    bytes_per_pixel: 1,
    allow_empty: true,
}];

static POSTERIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::native_byte_point_architectures(OperationId::Posterize, Mode::ImagePalette8, 1);
static SOLARIZE_ARCHITECTURES: [ImplementationSupport; 3] =
    strategy::native_byte_point_architectures(OperationId::Solarize, Mode::ImagePalette8, 1);

// NO_PILLOW_SUPPORT: Pillow's ImageOps._lut rejects P. These native strategies
// transform palette indices and preserve the Rust image's attached palette.
pub(crate) const POSTERIZE_SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Posterize,
    Mode::ImagePalette8,
    OutputModeRule::Preserve,
    POSTERIZE_INPUTS,
    &POSTERIZE_ARCHITECTURES,
    "Posterize applies its byte lookup table directly to native palette indices",
);

// NO_PILLOW_SUPPORT: Pillow's ImageOps._lut rejects P; this native strategy
// solarizes index bytes and preserves the Rust image's attached palette.
pub(crate) const SOLARIZE_SUPPORT: ModeSupport = ModeSupport::cpu_core_only(
    OperationId::Solarize,
    Mode::ImagePalette8,
    OutputModeRule::Preserve,
    SOLARIZE_INPUTS,
    &SOLARIZE_ARCHITECTURES,
    "Solarize applies its byte lookup table directly to native palette indices",
);
