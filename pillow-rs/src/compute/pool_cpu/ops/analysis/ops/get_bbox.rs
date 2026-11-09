//! GetBBox family support aggregation.

use crate::compute::registry::support::{ModeSupport, OperationId, OperationSupport};
use crate::raster::Mode;

fn classify_get_bbox_mode(mode: Mode) -> ModeSupport {
    match mode {
        Mode::One => super::super::one::ops::get_bbox_mode_support(),
        Mode::ImageLuma8 => super::super::luma8::ops::get_bbox_mode_support(),
        Mode::ImageLumaA8 => super::super::luma_a8::ops::get_bbox_mode_support(),
        Mode::ImageLaPremultiplied8 => {
            super::super::la_premultiplied8::ops::get_bbox_mode_support()
        }
        Mode::ImagePalette8 => super::super::palette8::ops::get_bbox_mode_support(),
        Mode::ImagePaletteAlpha8 => super::super::palette_alpha8::ops::get_bbox_mode_support(),
        Mode::ImageRgb8 => super::super::rgb8::ops::get_bbox_mode_support(),
        Mode::ImageRgba8 => super::super::rgba8::ops::get_bbox_mode_support(),
        Mode::ImageRgbPremultiplied8 => {
            super::super::rgb_premultiplied8::ops::get_bbox_mode_support()
        }
        Mode::ImageRgbx8 => super::super::rgbx8::ops::get_bbox_mode_support(),
        Mode::ImageCmyk8 => super::super::cmyk8::ops::get_bbox_mode_support(),
        Mode::ImageYCbCr8 => super::super::ycbcr8::ops::get_bbox_mode_support(),
        Mode::ImageLab8 => super::super::lab8::ops::get_bbox_mode_support(),
        Mode::ImageHsv8 => super::super::hsv8::ops::get_bbox_mode_support(),
        Mode::ImageI32 => super::super::i32::ops::get_bbox_mode_support(),
        Mode::ImageF32 => super::super::f32::ops::get_bbox_mode_support(),
        Mode::ImageI16 => super::super::i16::ops::get_bbox_mode_support(),
        Mode::ImageI16L => super::super::i16l::ops::get_bbox_mode_support(),
        Mode::ImageI16B => super::super::i16b::ops::get_bbox_mode_support(),
        Mode::ImageI16N => super::super::i16n::ops::get_bbox_mode_support(),
        Mode::ImageLumaA16 => super::super::luma_a16::ops::get_bbox_mode_support(),
        Mode::ImageRgb16 => super::super::rgb16::ops::get_bbox_mode_support(),
        Mode::ImageRgba16 => super::super::rgba16::ops::get_bbox_mode_support(),
        Mode::ImageRgb32F => super::super::rgb32f::ops::get_bbox_mode_support(),
        Mode::ImageRgba32F => super::super::rgba32f::ops::get_bbox_mode_support(),
    }
}

/// Exhaustive GetBBox support aggregated from exact-mode owners.
/// TODO(pillow-rs#3ptb): verify alpha-only inputs, bounds, and Default receipts.
pub(crate) static GET_BBOX_SUPPORT: OperationSupport =
    OperationSupport::new(OperationId::GetBBox, &[], classify_get_bbox_mode);
