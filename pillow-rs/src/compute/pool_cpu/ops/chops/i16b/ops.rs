//! Exact i16b Chops support and mode strategy.

use super::super::helpers::strategy::declare_i16_offset_architectures;
use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageI16B;

declare_i16_offset_architectures!(MODE);
const INVERT_BYTES_PER_PIXEL: u8 = 2;
static INVERT_ARCHITECTURES: [ImplementationSupport; 1] = [
    super::super::helpers::strategy::avx512f_typed_invert_architecture(
        MODE,
        INVERT_BYTES_PER_PIXEL,
        2,
    ),
];

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn support(operation: OperationId) -> ModeSupport {
    let architectures: &'static [ImplementationSupport] = if operation == OperationId::ChopsOffset {
        &OFFSET_ARCHITECTURES
    } else if operation == OperationId::ChopsInvert {
        &INVERT_ARCHITECTURES
    } else {
        &[]
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
    let DynamicImage::ImageI16B(buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    match request {
        super::super::ops::ChopsRequest::Invert => match architecture {
            Architecture::Default => super::default::ops::execute(image, request, other),
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            Architecture::Avx512F => {
                super::super::helpers::strategy::require_avx512f_typed_invert_selection(
                    selection,
                    MODE,
                    INVERT_BYTES_PER_PIXEL,
                    2,
                )?;
                super::avx512f::invert_impl::execute(buffer).map(DynamicImage::ImageI16B)
            }
            _ => Err(PilError::InternalError(
                "Chops I;16B Invert selected an unsupported architecture".into(),
            )),
        },
        super::super::ops::ChopsRequest::Offset { x, y } => match architecture {
            Architecture::Default => super::default::ops::execute(image, request, other),
            Architecture::Sse2 => {
                super::sse2::execute_offset(buffer, x, y).map(DynamicImage::ImageI16B)
            }
            Architecture::Neon => {
                super::neon::execute_offset(buffer, x, y).map(DynamicImage::ImageI16B)
            }
            Architecture::WasmSimd128 => {
                super::wasm_simd128::execute_offset(buffer, x, y).map(DynamicImage::ImageI16B)
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
