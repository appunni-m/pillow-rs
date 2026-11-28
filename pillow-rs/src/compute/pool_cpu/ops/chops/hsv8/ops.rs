//! Exact hsv8 Chops support and mode strategy.

use super::super::helpers::strategy::{
    avx512f_byte_invert_architecture, declare_byte_binary_architectures,
};
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageHsv8;
const CHANNELS: u8 = 3;

declare_byte_binary_architectures!(MODE, CHANNELS);
static INVERT_ARCHITECTURES: [ImplementationSupport; 1] =
    [avx512f_byte_invert_architecture(MODE, CHANNELS)];

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn support(operation: OperationId) -> ModeSupport {
    let architectures: &[_] = if operation == OperationId::ChopsInvert {
        &INVERT_ARCHITECTURES
    } else {
        architecture_for(operation)
    };
    super::super::ops::mode_support_with_architectures(MODE, operation, architectures)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let architecture = selection.implementation.architecture;
    if matches!(request, super::super::ops::ChopsRequest::Invert) {
        return match image {
            DynamicImage::ImageHsv8(_buffer) => match architecture {
                Architecture::Default => super::default::ops::execute(image, request, other),
                #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                Architecture::Avx512F => {
                    super::avx512f::invert_impl::execute(_buffer).map(DynamicImage::ImageHsv8)
                }
                _ => Err(PilError::InternalError(
                    "Chops HSV Invert selected an unsupported architecture".into(),
                )),
            },
            _ => Err(PilError::InternalError(
                "Chops mode strategy received a different raster variant".into(),
            )),
        };
    }
    if let super::super::ops::ChopsRequest::Offset { x, y } = request {
        return match image {
            DynamicImage::ImageHsv8(_buffer) => match architecture {
                Architecture::Default => super::default::ops::execute(image, request, other),
                Architecture::Sse2 => {
                    super::sse2::execute_offset(_buffer, x, y).map(DynamicImage::ImageHsv8)
                }
                Architecture::Neon => {
                    super::neon::execute_offset(_buffer, x, y).map(DynamicImage::ImageHsv8)
                }
                Architecture::WasmSimd128 => {
                    super::wasm_simd128::execute_offset(_buffer, x, y).map(DynamicImage::ImageHsv8)
                }
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
        DynamicImage::ImageHsv8(_buffer) => {
            if architecture == Architecture::Default {
                return super::default::ops::execute(image, request, other);
            }
            let other = match other {
                Some(
                    DynamicImage::ImageRgb8(other)
                    | DynamicImage::ImageYCbCr8(other)
                    | DynamicImage::ImageLab8(other)
                    | DynamicImage::ImageHsv8(other),
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
                Architecture::Sse2 => {
                    super::sse2::execute(_buffer, request, other).map(DynamicImage::ImageHsv8)
                }
                Architecture::Neon => {
                    super::neon::execute(_buffer, request, other).map(DynamicImage::ImageHsv8)
                }
                Architecture::WasmSimd128 => super::wasm_simd128::execute(_buffer, request, other)
                    .map(DynamicImage::ImageHsv8),
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

// VERIFY: source=reviewed@2026-11-21; parity=pending; benchmark=pending
pub(crate) fn execute_in_place(
    image: &mut DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: &DynamicImage,
    selection: &StrategySelection,
) -> Result<(), PilError> {
    let architecture = selection.implementation.architecture;
    match image {
        DynamicImage::ImageHsv8(_buffer) => {
            let other = match other {
                DynamicImage::ImageRgb8(other)
                | DynamicImage::ImageYCbCr8(other)
                | DynamicImage::ImageLab8(other)
                | DynamicImage::ImageHsv8(other) => other,
                _ => {
                    return Err(PilError::InternalError(
                        "Chops in-place strategy received a different secondary raster variant"
                            .into(),
                    ));
                }
            };
            match architecture {
                Architecture::Sse2 => super::sse2::execute_in_place(_buffer, request, other),
                Architecture::Neon => super::neon::execute_in_place(_buffer, request, other),
                Architecture::WasmSimd128 => {
                    super::wasm_simd128::execute_in_place(_buffer, request, other)
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
