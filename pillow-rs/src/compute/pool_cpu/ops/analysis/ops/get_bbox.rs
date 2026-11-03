//! GetBBox mode support contract.

use crate::compute::registry::support::{
    InputContract, ModeSupport, OperationId, OperationSupport,
};
use crate::raster::Mode;

use super::NO_RASTER_OUTPUT;

const GET_BBOX_ALPHA_MODES: &[Mode] = &[
    Mode::ImageLumaA8,
    Mode::ImageLaPremultiplied8,
    Mode::ImagePaletteAlpha8,
    Mode::ImageRgba8,
    Mode::ImageRgbPremultiplied8,
    Mode::ImageLumaA16,
    Mode::ImageRgba16,
    Mode::ImageRgba32F,
];
const GET_BBOX_INPUTS: &[InputContract] = &[InputContract::GetBBoxAlphaOnly {
    alpha_modes: GET_BBOX_ALPHA_MODES,
}];

fn classify_get_bbox_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::ImageLumaA16
        | Mode::ImageRgb16
        | Mode::ImageRgba16
        | Mode::ImageRgb32F
        | Mode::ImageRgba32F => ModeSupport::cpu_core_only(
            OperationId::GetBBox,
            mode,
            NO_RASTER_OUTPUT,
            GET_BBOX_INPUTS,
            &[],
            "native high-depth GetBBox extends the Pillow mode contract",
        ),
        Mode::One
        | Mode::ImageLuma8
        | Mode::ImageLumaA8
        | Mode::ImageLaPremultiplied8
        | Mode::ImagePalette8
        | Mode::ImagePaletteAlpha8
        | Mode::ImageRgb8
        | Mode::ImageRgba8
        | Mode::ImageRgbPremultiplied8
        | Mode::ImageRgbx8
        | Mode::ImageCmyk8
        | Mode::ImageYCbCr8
        | Mode::ImageLab8
        | Mode::ImageHsv8
        | Mode::ImageI32
        | Mode::ImageF32
        | Mode::ImageI16
        | Mode::ImageI16L
        | Mode::ImageI16B
        | Mode::ImageI16N => ModeSupport::cpu(
            OperationId::GetBBox,
            mode,
            NO_RASTER_OUTPUT,
            GET_BBOX_INPUTS,
            &[],
        ),
    }
}

/// Exhaustive GetBBox support, including the exact alpha-bearing mode set.
// TODO(pillow-rs#3ptb): cover all 25 modes under Default and every non-Default
// Architecture request; verify Default fallback, I;16* high-byte samples, and
// alpha-only mode selection in the CPU parity matrix.
pub(crate) static GET_BBOX_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetBBox, &[], classify_get_bbox_mode);
