//! Exact i16b Chops support and mode strategy.

use crate::compute::registry::support::{ImplementationSupport, ModeSupport, OperationId};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::super::helpers::strategy::declare_i16_offset_architectures;

pub(crate) const MODE: Mode = Mode::ImageI16B;

declare_i16_offset_architectures!(MODE);

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
    architecture: Architecture,
) -> Result<DynamicImage, PilError> {
    let DynamicImage::ImageI16B(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    match request {
        super::super::ops::ChopsRequest::Offset { x, y } => match architecture {
            Architecture::Default => default::ops::execute(image, request, other),
            Architecture::Sse2 => super::sse2::execute_offset(buffer, x, y, true)
                .map(DynamicImage::ImageI16B),
            Architecture::Neon => super::neon::execute_offset(buffer, x, y, true)
                .map(DynamicImage::ImageI16B),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::execute_offset(buffer, x, y, true)
                    .map(DynamicImage::ImageI16B)
            }
            _ => Err(PilError::InternalError(
                "Chops Offset selected an unsupported architecture".into(),
            )),
        },
        _ if architecture == Architecture::Default => default::ops::execute(image, request, other),
        _ => Err(PilError::InternalError(
            "Chops mode strategy selected an architecture without an implementation".into(),
        )),
    }
}
