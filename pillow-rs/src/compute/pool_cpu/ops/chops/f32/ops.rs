//! Exact f32 Chops support and mode strategy.

use super::super::helpers::strategy::declare_byte_offset_architectures;
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageF32;
const CHANNELS: u8 = 4;

declare_byte_offset_architectures!(MODE, CHANNELS);

pub(crate) fn support(operation: OperationId) -> ModeSupport {
    let architectures: &'static [ImplementationSupport] = if operation == OperationId::ChopsOffset {
        &OFFSET_ARCHITECTURES
    } else {
        &[]
    };
    super::super::ops::mode_support_with_architectures(MODE, operation, architectures)
}

pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    let architecture = selection.implementation.architecture;
    let DynamicImage::ImageF32(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    match request {
        super::super::ops::ChopsRequest::Offset { x, y } => match architecture {
            Architecture::Default => super::default::ops::execute(image, request, other),
            Architecture::Sse2 => {
                super::sse2::execute_offset(buffer, x, y).map(DynamicImage::ImageF32)
            }
            Architecture::Neon => {
                super::neon::execute_offset(buffer, x, y).map(DynamicImage::ImageF32)
            }
            Architecture::WasmSimd128 => {
                super::wasm_simd128::execute_offset(buffer, x, y).map(DynamicImage::ImageF32)
            }
            _ => Err(PilError::InternalError(
                "Chops Offset selected an unsupported architecture".into(),
            )),
        },
        _ if architecture == Architecture::Default => {
            super::default::ops::execute(image, request, other)
        }
        _ => Err(PilError::InternalError(
            "Chops mode strategy selected an architecture without an implementation".into(),
        )),
    }
}
