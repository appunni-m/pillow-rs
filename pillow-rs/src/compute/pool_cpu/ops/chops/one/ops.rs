//! Exact one Chops support and mode strategy.

use crate::compute::registry::support::{ImplementationSupport, ModeSupport, OperationId};
use crate::compute::Architecture;
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};
use super::super::helpers::strategy::{
    declare_byte_binary_architectures, declare_byte_invert_architectures,
    declare_logical_architectures,
};

pub(crate) const MODE: Mode = Mode::One;
const CHANNELS: u8 = 1;

declare_byte_binary_architectures!(MODE, CHANNELS);
declare_byte_invert_architectures!(MODE, CHANNELS);
declare_logical_architectures!(MODE);

pub(crate) fn support(operation: OperationId) -> ModeSupport {
    let architectures = match operation {
        OperationId::ChopsLogicalAnd => &LOGICAL_AND_ARCHITECTURES,
        OperationId::ChopsLogicalOr => &LOGICAL_OR_ARCHITECTURES,
        OperationId::ChopsLogicalXor => &LOGICAL_XOR_ARCHITECTURES,
        OperationId::ChopsInvert => &INVERT_ARCHITECTURES,
        _ => binary_architectures_for(operation),
    };
    super::super::ops::mode_support_with_architectures(MODE, operation, architectures)
}

pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
    architecture: Architecture,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::One(_) => match architecture {
            Architecture::Default => default::ops::execute(image, request, other),
            Architecture::Sse2 => super::sse2::execute(image, request, other, MODE),
            Architecture::Neon => super::neon::execute(image, request, other, MODE),
            Architecture::WasmSimd128 => super::wasm_simd128::execute(image, request, other, MODE),
            _ => Err(PilError::InternalError(
                "Chops vector strategy received an unsupported architecture".into(),
            )),
        },
        _ => Err(PilError::InternalError("Chops mode strategy received a different raster variant".into())),
    }
}

pub(crate) fn execute_in_place(
    image: &mut DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: &DynamicImage,
    architecture: Architecture,
) -> Result<(), PilError> {
    match image {
        DynamicImage::One(_) => match architecture {
            Architecture::Sse2 => super::sse2::execute_in_place(image, request, other, MODE),
            Architecture::Neon => super::neon::execute_in_place(image, request, other, MODE),
            Architecture::WasmSimd128 => {
                super::wasm_simd128::execute_in_place(image, request, other, MODE)
            }
            _ => Err(PilError::InternalError(
                "Chops in-place strategy received an unsupported architecture".into(),
            )),
        },
        _ => Err(PilError::InternalError(
            "Chops in-place mode strategy received a different raster variant".into(),
        )),
    }
}
