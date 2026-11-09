//! Pad contract and architecture selection for ImageYCbCr8.

use crate::compute::pool_cpu::ops::geometry::pad::helpers::dimensions::PadFrame;
use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategySelection,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

static ARCHITECTURES: [ImplementationSupport; 3] =
    crate::compute::pool_cpu::ops::geometry::pad::helpers::strategies::byte_vector_architectures(
        Mode::ImageYCbCr8,
        3,
    );

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Pad,
    Mode::ImageYCbCr8,
    OutputModeRule::Preserve,
    super::super::ops::MODE_ENCODED_FILL_INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn pad(
    image: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let DynamicImage::ImageYCbCr8(image) = image else {
        return Err(PilError::InternalError(
            "Pad mode dispatch received a different raster".into(),
        ));
    };
    let architecture =
        super::super::helpers::strategies::selected_architecture(selection, Mode::ImageYCbCr8)?;
    match architecture {
        Architecture::Default => super::default::pad_impl::pad(image, frame)
            .map(|output| (DynamicImage::ImageYCbCr8(output), 0, 0)),
        Architecture::Sse2 => super::sse2::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImageYCbCr8(output), blocks, tail)),
        Architecture::Neon => super::neon::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImageYCbCr8(output), blocks, tail)),
        Architecture::WasmSimd128 => super::wasm_simd128::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImageYCbCr8(output), blocks, tail)),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for ImageYCbCr8 Pad"
        ))),
    }
}
