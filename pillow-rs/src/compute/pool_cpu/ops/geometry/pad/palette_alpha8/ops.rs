//! Pad contract and architecture selection for ImagePaletteAlpha8.

use crate::compute::{Architecture, registry::support::{
    ImplementationSupport, ModeSupport, OperationId, OutputModeRule,
}};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::PadFrame;

static ARCHITECTURES: [ImplementationSupport; 3] =
    crate::compute::pool_cpu::ops::geometry::pad::helpers::strategies::byte_vector_architectures(
        Mode::ImagePaletteAlpha8, 2,
    );

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Pad,
    Mode::ImagePaletteAlpha8,
    OutputModeRule::Preserve,
    super::super::ops::MODE_ENCODED_FILL_INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn pad(
    image: &DynamicImage,
    frame: PadFrame,
    architecture: Architecture,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let DynamicImage::ImagePaletteAlpha8(image) = image else {
        return Err(PilError::InternalError("Pad mode dispatch received a different raster".into()));
    };
    match architecture {
        Architecture::Default => super::default::pad_impl::pad(image, frame)
            .map(|output| (DynamicImage::ImagePaletteAlpha8(output), 0, 0)),
        Architecture::Sse2 => sse2::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImagePaletteAlpha8(output), blocks, tail)),
        Architecture::Neon => neon::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImagePaletteAlpha8(output), blocks, tail)),
        Architecture::WasmSimd128 => wasm_simd128::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImagePaletteAlpha8(output), blocks, tail)),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for ImagePaletteAlpha8 Pad"
        ))),
    }
}
