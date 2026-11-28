//! Exact lab8 Chops support and mode strategy.

use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageLab8;
const CHANNELS: u8 = 3;
static INVERT_ARCHITECTURES: [ImplementationSupport; 1] =
    [super::super::helpers::strategy::avx512f_byte_invert_architecture(MODE, CHANNELS)];

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
// Scope: LAB ChopsAdd is Default-only and has no vector in-place route.
pub(crate) fn support(operation: OperationId) -> ModeSupport {
    let architectures: &[_] = if operation == OperationId::ChopsInvert {
        &INVERT_ARCHITECTURES
    } else {
        &[]
    };
    super::super::ops::mode_support_with_architectures(MODE, operation, architectures)
}

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
// Scope: LAB ChopsAdd enforces its declared Default strategy.
pub(crate) fn execute(
    image: &DynamicImage,
    request: super::super::ops::ChopsRequest,
    other: Option<&DynamicImage>,
    selection: &StrategySelection,
) -> Result<DynamicImage, PilError> {
    match image {
        DynamicImage::ImageLab8(_buffer) => match request {
            super::super::ops::ChopsRequest::Invert => {
                match selection.implementation.architecture {
                    Architecture::Default => super::default::ops::execute(image, request, other),
                    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
                    Architecture::Avx512F => {
                        super::avx512f::invert_impl::execute(_buffer).map(DynamicImage::ImageLab8)
                    }
                    _ => Err(PilError::InternalError(
                        "Chops LAB Invert selected an unsupported architecture".into(),
                    )),
                }
            }
            _ => {
                super::super::helpers::strategy::require_default_selection(selection, MODE)?;
                super::default::ops::execute(image, request, other)
            }
        },
        _ => Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        )),
    }
}
