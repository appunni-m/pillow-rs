//! Exact rgb32f Chops support and mode strategy.

use crate::compute::Architecture;
use crate::compute::registry::support::{
    ImplementationSupport, ModeSupport, OperationId, StrategySelection,
};
use crate::error::PilError;
use crate::raster::{DynamicImage, Mode};

pub(crate) const MODE: Mode = Mode::ImageRgb32F;
const INVERT_BYTES_PER_PIXEL: u8 = 12;
const INVERT_SAMPLE_BYTES: u8 = 4;
static INVERT_ARCHITECTURES: [ImplementationSupport; 1] = [
    super::super::helpers::strategy::avx512f_typed_invert_architecture(
        MODE,
        INVERT_BYTES_PER_PIXEL,
        INVERT_SAMPLE_BYTES,
    ),
];

// VERIFY: source=reviewed@2026-11-25; parity=pending; benchmark=pending
pub(crate) fn support(operation: OperationId) -> ModeSupport {
    let architectures: &[_] = if operation == OperationId::ChopsInvert {
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
    let DynamicImage::ImageRgb32F(_buffer) = image else {
        return Err(PilError::InternalError(
            "Chops mode strategy received a different raster variant".into(),
        ));
    };
    match request {
        super::super::ops::ChopsRequest::Invert => match selection.implementation.architecture {
            Architecture::Default => super::default::ops::execute(image, request, other),
            #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
            Architecture::Avx512F => {
                super::super::helpers::strategy::require_avx512f_typed_invert_selection(
                    selection,
                    MODE,
                    INVERT_BYTES_PER_PIXEL,
                    INVERT_SAMPLE_BYTES,
                )?;
                super::avx512f::invert_impl::execute(_buffer).map(DynamicImage::ImageRgb32F)
            }
            _ => Err(PilError::InternalError(
                "Chops RGB32F Invert selected an unsupported architecture".into(),
            )),
        },
        _ => {
            super::super::helpers::strategy::require_default_selection(selection, MODE)?;
            super::default::ops::execute(image, request, other)
        }
    }
}
