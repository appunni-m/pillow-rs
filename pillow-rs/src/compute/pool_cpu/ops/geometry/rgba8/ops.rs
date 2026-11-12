//! Pad contract and architecture selection for ImageRgba8.

use crate::compute::pool_cpu::ops::geometry::helpers::pad_dimensions::PadFrame;
use crate::compute::{
    Architecture,
    registry::support::{
        ImplementationSupport, ModeSupport, OperationId, OutputModeRule, StrategySelection,
    },
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

static ARCHITECTURES: [ImplementationSupport; 3] =
    crate::compute::pool_cpu::ops::geometry::helpers::pad_strategies::byte_vector_architectures(
        Mode::ImageRgba8,
        4,
    );

pub(crate) const SUPPORT: ModeSupport = ModeSupport::cpu(
    OperationId::Pad,
    Mode::ImageRgba8,
    OutputModeRule::Preserve,
    super::super::ops::MODE_ENCODED_FILL_INPUTS,
    &ARCHITECTURES,
);

pub(crate) fn pad(
    image: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
) -> Result<(DynamicImage, u64, u64), PilError> {
    let DynamicImage::ImageRgba8(image) = image else {
        return Err(PilError::InternalError(
            "Pad mode dispatch received a different raster".into(),
        ));
    };
    let architecture =
        super::super::helpers::pad_strategies::selected_architecture(selection, Mode::ImageRgba8)?;
    match architecture {
        Architecture::Default => super::default::pad_impl::pad(image, frame)
            .map(|output| (DynamicImage::ImageRgba8(output), 0, 0)),
        Architecture::Sse2 => super::sse2::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImageRgba8(output), blocks, tail)),
        Architecture::Neon => super::neon::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImageRgba8(output), blocks, tail)),
        Architecture::WasmSimd128 => super::wasm_simd128::pad_impl::pad(image, frame)
            .map(|(output, blocks, tail)| (DynamicImage::ImageRgba8(output), blocks, tail)),
        architecture => Err(PilError::InternalError(format!(
            "unexpected {architecture:?} architecture selected for ImageRgba8 Pad"
        ))),
    }
}

pub(crate) fn try_vertical_append(
    raster: &DynamicImage,
    frame: PadFrame,
    selection: &StrategySelection,
    content_source: super::super::ops::PadContentSource,
) -> Result<Option<(DynamicImage, u64, u64)>, PilError> {
    use super::super::ops::PadContentSource::Input;

    let architecture =
        super::super::helpers::pad_strategies::selected_architecture(selection, Mode::ImageRgba8)?;
    let DynamicImage::ImageRgba8(image) = raster else {
        return Err(PilError::InternalError(
            "ImageRgba8 Pad strategy received a different raster".into(),
        ));
    };
    match (content_source, architecture) {
        (Input, Architecture::Default) => super::super::helpers::pad_samples::pad_vertical_append(
            image,
            frame,
            super::default::pad_impl::fill(frame.color),
        )
        .map(|output| output.map(|output| (DynamicImage::ImageRgba8(output), 0, 0))),
        (Input, Architecture::Sse2 | Architecture::Neon | Architecture::WasmSimd128) => {
            super::super::helpers::pad_native_vector::pad_vertical_append::<_, true>(
                image,
                frame,
                super::default::pad_impl::fill(frame.color),
            )
            .map(|output| {
                output
                    .map(|(output, blocks, tail)| (DynamicImage::ImageRgba8(output), blocks, tail))
            })
        }
        _ => Ok(None),
    }
}
