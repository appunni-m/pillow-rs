//! Exact rgb_premultiplied8 Chops support and mode strategy.

use super::super::helpers::strategy::declare_byte_binary_architectures;
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageRgbPremultiplied8;
const CHANNELS: u8 = 4;

declare_byte_binary_architectures!(MODE, CHANNELS);

pub(crate) fn support(operation: OperationId) -> ModeSupport {
    let architectures = architecture_for(operation);
    super::super::ops::mode_support_with_architectures(MODE, operation, architectures)
}

pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let architecture = selection.implementation.architecture;
    if let super::super::ops::ChopsRequest::Offset { x, y } = request {
        return match image {
            DynamicImage::ImageRgbPremultiplied8(buffer) => match architecture {
                Architecture::Default => super::default::ops::execute(image, request, other),
                Architecture::Sse2 => super::sse2::execute_offset(buffer, x, y)
                    .map(DynamicImage::ImageRgbPremultiplied8),
                Architecture::Neon => super::neon::execute_offset(buffer, x, y)
                    .map(DynamicImage::ImageRgbPremultiplied8),
                Architecture::WasmSimd128 => super::wasm_simd128::execute_offset(buffer, x, y)
                    .map(DynamicImage::ImageRgbPremultiplied8),
                _ => Err(PilError::InternalError(
                    "Chops Offset selected an unsupported architecture".into(),
                )),
            },
            _ => Err(PilError::InternalError(
                "Chops mode strategy received a different raster variant".into(),
            )),
        };
    }

    match image {
        DynamicImage::ImageRgbPremultiplied8(buffer) => {
            if architecture == Architecture::Default {
                return super::default::ops::execute(image, request, other);
            }
            let other = match other {
                Some(
                    DynamicImage::ImageRgba8(other)
                    | DynamicImage::ImageCmyk8(other)
                    | DynamicImage::ImageRgbPremultiplied8(other)
                    | DynamicImage::ImageRgbx8(other),
                ) => Some(other),
                None => None,
                _ => {
                    return Err(PilError::InternalError(
                        "Chops vector strategy received a different secondary raster variant"
                            .into(),
                    ));
                }
            };
            match architecture {
                Architecture::Sse2 => super::sse2::execute(buffer, request, other)
                    .map(DynamicImage::ImageRgbPremultiplied8),
                Architecture::Neon => super::neon::execute(buffer, request, other)
                    .map(DynamicImage::ImageRgbPremultiplied8),
                Architecture::WasmSimd128 => super::wasm_simd128::execute(buffer, request, other)
                    .map(DynamicImage::ImageRgbPremultiplied8),
                _ => Err(PilError::InternalError(
                    "Chops vector strategy received an unsupported architecture".into(),
                )),
            }
        }
        _ => Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        )),
    }
}

pub(crate) fn execute_in_place(
    image: &mut DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: &DynamicImage,
    selection: &StrategySelection,
) -> Result<(), PilError> {
    let architecture = selection.implementation.architecture;
    match image {
        DynamicImage::ImageRgbPremultiplied8(buffer) => {
            let other = match other {
                DynamicImage::ImageRgba8(other)
                | DynamicImage::ImageCmyk8(other)
                | DynamicImage::ImageRgbPremultiplied8(other)
                | DynamicImage::ImageRgbx8(other) => other,
                _ => {
                    return Err(PilError::InternalError(
                        "Chops in-place strategy received a different secondary raster variant"
                            .into(),
                    ));
                }
            };
            match architecture {
                Architecture::Sse2 => super::sse2::execute_in_place(buffer, request, other),
                Architecture::Neon => super::neon::execute_in_place(buffer, request, other),
                Architecture::WasmSimd128 => {
                    super::wasm_simd128::execute_in_place(buffer, request, other)
                }
                _ => Err(PilError::InternalError(
                    "Chops in-place strategy received an unsupported architecture".into(),
                )),
            }
        }
        _ => Err(PilError::InternalError(
            "Chops in-place mode strategy received a different raster variant".into(),
        )),
    }
}
